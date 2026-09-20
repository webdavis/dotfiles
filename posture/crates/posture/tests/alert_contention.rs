//! Two `posture alert` runs over one results log, as a WatchPaths burst fires
//! them. The single-instance lock only means anything across real processes,
//! so both halves of this are separate children of the built binary.

mod sandbox;

use sandbox::Sandbox;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A LIVENESS BOUND, NOT AN ASSERTION. The row pins the one call and the one
/// cursor, never how long two runs took on a machine compiling other lanes.
const LIVENESS_BOUND: Duration = Duration::from_secs(15);

#[test]
fn two_parallel_runs_deliver_one_batch_once_and_share_its_final_cursor() {
    let sandbox = Sandbox::new("alert-contention");
    let home = sandbox.path();
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
printf '{{"schema":"pns.result/1","request_id":"%s","status":"delivered","diagnostics":["ledger_committed"]}}\n' "$identity"
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
    let mut children: Vec<_> = (0..2)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_posture"))
                .env_clear()
                .env("HOME", home)
                .env("TMPDIR", home)
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

    // Both children are driven to exit (or killed) before anything asserts, so
    // a failure on one never leaves its sibling running against the sandbox.
    let mut statuses: Vec<Option<std::process::ExitStatus>> = vec![None; children.len()];
    loop {
        for (child, status) in children.iter_mut().zip(statuses.iter_mut()) {
            if status.is_none()
                && let Ok(Some(s)) = child.try_wait()
            {
                *status = Some(s);
            }
        }
        if statuses.iter().all(Option::is_some) {
            break;
        }
        if Instant::now() >= deadline {
            for child in &mut children {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!("alert exceeded its bound: {statuses:?}");
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    let calls_content = std::fs::read_to_string(&calls).unwrap();
    let cursor_content = std::fs::read_to_string(&cursor).unwrap();

    for status in statuses.into_iter().flatten() {
        assert_eq!(status.code(), Some(0), "a contended run is a clean no-op");
    }
    assert_eq!(
        calls_content, "call\n",
        "the batch reached the engine exactly once"
    );
    assert_eq!(
        cursor_content.trim(),
        format!("{inode} {}", batch.len()),
        "both runs leave one shared final cursor at the end of the batch"
    );
}
