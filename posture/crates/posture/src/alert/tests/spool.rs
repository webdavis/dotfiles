use super::*;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio};
use std::time::Instant;

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
    let root = std::env::temp_dir().join(format!(
        "posture-spool-diagnostic-{}-{case}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "alert::tests::spool::private_alert_fixture",
            "--ignored",
            "--nocapture",
        ])
        .env_clear()
        .env("HOME", &root)
        .env("TMPDIR", &root)
        .env("POSTURE_SPOOL_FIXTURE", case)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(650);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1));
            }
            state => {
                let killed = child.kill();
                let reaped = child.wait();
                panic!("private alert exceeded its bound: {state:?}, {killed:?}, {reaped:?}");
            }
        }
    }
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{:?}", result.stderr);
    assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    let spool = root.join("spool/digest.ndjson");
    if case == "writable" {
        assert!(result.stderr.is_empty(), "{:?}", result.stderr);
        assert!(std::fs::read_to_string(&spool).unwrap().contains(FINDING));
    } else {
        let diagnostic = String::from_utf8(result.stderr).unwrap();
        assert!(diagnostic.contains(&spool.to_string_lossy().to_string()));
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
    assert_eq!(std::fs::read(root.join("calls")).unwrap(), b"call\n");
    let request = std::fs::read_to_string(root.join("request")).unwrap();
    assert!(request.contains("later-admin"), "{request}");
    assert!(!request.contains(FINDING), "{request}");
    let log = std::fs::metadata(root.join("results")).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("cursor")).unwrap(),
        format!("{} {}\n", log.ino(), log.len())
    );
}

#[test]
#[ignore = "private subprocess fixture"]
fn private_alert_fixture() {
    let Ok(case) = std::env::var("POSTURE_SPOOL_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let mut config =
        Configuration::read(|name| (name == "HOME").then(|| root.clone().into())).unwrap();
    config.log = root.join("results");
    config.cursor = root.join("cursor");
    config.spool = root.join("spool/digest.ndjson");
    config.pipeline_manifest = root.join("pipeline-manifest");
    config.managed_bin_manifest = root.join("managed-manifest");
    config.alarm = root.join("unused-private-alarm");
    match case.as_str() {
        "parent" => std::fs::write(root.join("spool"), b"blocked parent").unwrap(),
        "open" => std::fs::create_dir_all(&config.spool).unwrap(),
        "writable" => {}
        _ => panic!("unknown private fixture"),
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
    std::fs::create_dir_all(config.pns.parent().unwrap()).unwrap();
    std::fs::write(&config.pns, br##"#!/bin/sh
set -eu
[ "$#" = 2 ] && [ "$1" = submit ] && [ "$2" = --json ] || exit 42
IFS= read -r request
printf '%s\n' "$request" >"$HOME/request"
printf 'call\n' >>"$HOME/calls"
identity="$(printf '%s' "$request" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
printf '{"schema":"pns.result/1","request_id":"%s","status":"accepted","diagnostics":["ledger_committed"]}\n' "$identity"
"##).unwrap();
    std::fs::set_permissions(&config.pns, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        execute(config, Time, || NoInspection, &mut std::io::stderr()),
        0
    );
}
