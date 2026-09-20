//! A spool that cannot be written costs tomorrow's summary line and nothing
//! else: the page still goes out, the cursor still advances, and the complaint
//! names the store's own path and no part of the finding.
//!
//! IT GOES TO THE CALLER'S SINK, which is what lets this run in process. Every
//! other diagnostic on this path is written through the `stderr` argument
//! `execute` threads down, so asserting on a `Vec<u8>` is the whole test.

use super::*;
use std::os::unix::fs::MetadataExt;

const FINDING: &str = "private-finding-identity";
const SECRET: &str = "private-finding-secret";

/// A temp tree cleared before it is made and removed when the test ends.
///
/// CLEARED ON THE WAY IN as well as out, because the name is keyed on the
/// process and a run that died mid-test would otherwise hand the next one a
/// directory it refuses to recreate.
struct Root {
    path: std::path::PathBuf,
}

impl Root {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "posture-spool-diagnostic-{}-{case}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn a_failed_spool_directory_reports_only_its_path_and_continues_detection() {
    check("parent");
}

#[test]
fn a_failed_spool_open_reports_only_its_path_and_continues_detection() {
    check("open");
}

#[test]
fn a_successful_spool_append_stays_silent_and_continues_detection() {
    check("writable");
}

fn check(case: &str) {
    let root = Root::new(case);
    let request = root.path.join("request");
    let calls = root.path.join("calls");
    let mut config =
        Configuration::read(|name| (name == "HOME").then(|| root.path.clone().into())).unwrap();
    config.log = root.path.join("results");
    config.cursor = root.path.join("cursor");
    config.spool = root.path.join("spool/digest.ndjson");
    config.pipeline_manifest = root.path.join("pipeline-manifest");
    config.managed_bin_manifest = root.path.join("managed-manifest");
    config.alarm = root.path.join("unused-private-alarm");
    match case {
        "parent" => std::fs::write(root.path.join("spool"), b"blocked parent").unwrap(),
        "open" => std::fs::create_dir_all(&config.spool).unwrap(),
        "writable" => {}
        _ => panic!("unknown spool case"),
    }
    std::fs::write(
        &config.log,
        format!(
            "{{\"name\":\"agent_authfile_changed\",\"action\":\"added\",\"counter\":4,\"columns\":{{\"path\":\"{FINDING}\",\"secret\":\"{SECRET}\",\"sha256\":\"raw-finding-hash\"}}}}\n{{\"name\":\"new_admin_user\",\"action\":\"added\",\"counter\":4,\"columns\":{{\"username\":\"later-admin\"}}}}\n"
        ),
    )
    .unwrap();
    let inode = std::fs::metadata(&config.log).unwrap().ino();
    std::fs::write(&config.cursor, format!("{inode} 0\n")).unwrap();
    let engine = root.path.join(".local/libexec/engine");
    config.notify = crate::command_notify(&engine);
    std::fs::create_dir_all(engine.parent().unwrap()).unwrap();
    // THE PATHS ARE BAKED IN rather than read from `$HOME`, because this stub
    // inherits the test runner's environment instead of a cleared one.
    std::fs::write(
        &engine,
        format!(
            r##"#!/bin/sh
set -eu
[ "$#" = 2 ] && [ "$1" = send ] && [ "$2" = --json ] || exit 42
IFS= read -r request
printf '%s\n' "$request" >'{request}'
printf 'call\n' >>'{calls}'
identity="$(printf '%s' "$request" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
printf '{{"schema":"pns.result/1","request_id":"%s","status":"delivered","diagnostics":["ledger_committed"]}}\n' "$identity"
"##,
            request = request.display(),
            calls = calls.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700)).unwrap();

    let mut diagnostics = Vec::new();
    assert_eq!(execute(config, Time, || NoInspection, &mut diagnostics), 0);

    let spool = root.path.join("spool/digest.ndjson");
    let diagnostic = String::from_utf8(diagnostics).unwrap();
    if case == "writable" {
        assert!(diagnostic.is_empty(), "{diagnostic}");
        assert!(std::fs::read_to_string(&spool).unwrap().contains(FINDING));
    } else {
        assert!(
            diagnostic.contains(&spool.to_string_lossy().to_string()),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("could not append"), "{diagnostic}");
        assert_eq!(diagnostic.lines().count(), 1, "{diagnostic}");
        for private in [
            FINDING,
            SECRET,
            "raw-finding-hash",
            "agent_authfile_changed",
        ] {
            assert!(!diagnostic.contains(private), "{diagnostic}");
        }
    }
    assert_eq!(std::fs::read(&calls).unwrap(), b"call\n");
    let request = std::fs::read_to_string(&request).unwrap();
    assert!(request.contains("later-admin"), "{request}");
    assert!(!request.contains(FINDING), "{request}");
    let log = std::fs::metadata(root.path.join("results")).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path.join("cursor")).unwrap(),
        format!("{} {}\n", log.ino(), log.len())
    );
}
