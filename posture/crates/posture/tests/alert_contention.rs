//! Two `posture alert` runs over one results log, as a WatchPaths burst fires
//! them. The single-instance lock only means anything across real processes,
//! so both halves of this are separate children of the built binary.

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A LIVENESS BOUND, NOT AN ASSERTION. The row pins the one call and the one
/// cursor, never how long two runs took on a machine compiling other lanes.
const LIVENESS_BOUND: Duration = Duration::from_secs(15);

#[test]
fn two_parallel_runs_deliver_one_batch_once_and_share_its_final_cursor() {
    let home = std::env::temp_dir().join(format!(
        "posture-alert-contention-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let engine = home.join(".local/libexec/engine");
    let log = home.join(".local/log/osquery/osqueryd.results.log");
    let cursor = home.join(".local/state/osquery-results-offset");
    let config = home.join(".config/posture/config.toml");
    let calls = home.join("calls");
    for path in [&engine, &log, &cursor, &config] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    std::fs::write(
        &config,
        format!(
            "[notify]\nmode = \"command\"\n\n[notify.command]\npath = \"{}\"\n",
            engine.display()
        ),
    )
    .unwrap();
    std::fs::write(
        &engine,
        format!(
            r##"#!/bin/sh
set -eu
IFS= read -r request
printf 'call\n' >>'{}'
identity="$(printf '%s' "$request" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
printf '{{"schema":"pns.result/1","request_id":"%s","status":"accepted","diagnostics":["ledger_committed"]}}\n' "$identity"
"##,
            calls.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700)).unwrap();
    let batch =
        b"{\"name\":\"new_admin_user\",\"action\":\"added\",\"counter\":4,\"columns\":{\"username\":\"mallory\"}}\n";
    std::fs::write(&log, batch).unwrap();
    // THE CURSOR IS SEEDED AT THE HEAD OF THIS LOG. A run that finds no cursor
    // at all replays and pages about the reset first, which is a second
    // delivery that says nothing about contention.
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&log).unwrap().ino()
    };
    std::fs::write(&cursor, format!("{inode} 0\n")).unwrap();

    let deadline = Instant::now() + LIVENESS_BOUND;
    let children: Vec<_> = (0..2)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_posture"))
                .env_clear()
                .env("HOME", &home)
                .env("TMPDIR", &home)
                // The manifests are absolute by default, so they are pointed
                // inside the sandbox rather than left on the real machine's.
                .env("OSQUERY_PIPELINE_MANIFEST", home.join("pipeline-manifest"))
                .env(
                    "OSQUERY_MANAGED_BIN_MANIFEST",
                    home.join("managed-manifest"),
                )
                .arg("alert")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut child in children {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    assert_eq!(status.code(), Some(0), "a contended run is a clean no-op");
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                state => {
                    let killed = child.kill();
                    panic!("alert exceeded its bound: {state:?}, {killed:?}");
                }
            }
        }
    }

    assert_eq!(
        std::fs::read_to_string(&calls).unwrap(),
        "call\n",
        "the batch reached the engine exactly once"
    );
    assert_eq!(
        std::fs::read_to_string(&cursor).unwrap().trim(),
        format!("{inode} {}", batch.len()),
        "both runs leave one shared final cursor at the end of the batch"
    );
    let _ = std::fs::remove_dir_all(&home);
}
