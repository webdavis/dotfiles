//! A spool that cannot be written costs tomorrow's summary line and nothing
//! else: the page still goes out, the cursor still advances, and the complaint
//! names the store's own path and no part of the finding.
//!
//! IT GOES TO THE CALLER'S SINK, which is what lets this run in process. Every
//! other diagnostic on this path is written through the `stderr` argument
//! `execute` threads down, so asserting on a `Vec<u8>` is the whole test.

use super::*;
use crate::test_sandbox::Sandbox;
use std::os::unix::fs::MetadataExt;

const FINDING: &str = "private-finding-identity";
const SECRET: &str = "private-finding-secret";

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
    let sandbox = Sandbox::new(&format!("spool-diagnostic-{case}"));
    let root = sandbox.path();
    let request = root.join("request");
    let calls = root.join("calls");
    let mut config =
        Configuration::read(|name| (name == "HOME").then(|| root.to_path_buf().into())).unwrap();
    config.log = root.join("results");
    config.cursor = root.join("cursor");
    config.spool = root.join("spool/digest.ndjson");
    config.pipeline_manifest = root.join("pipeline-manifest");
    config.managed_bin_manifest = root.join("managed-manifest");
    config.alarm = root.join("unused-private-alarm");
    match case {
        "parent" => std::fs::write(root.join("spool"), b"blocked parent").unwrap(),
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
    let engine = root.join(".local/libexec/engine");
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

    let spool = root.join("spool/digest.ndjson");
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
    let log = std::fs::metadata(root.join("results")).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("cursor")).unwrap(),
        format!("{} {}\n", log.ino(), log.len())
    );
}
