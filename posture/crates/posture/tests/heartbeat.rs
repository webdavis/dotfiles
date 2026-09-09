use std::ffi::OsString;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn heartbeat_ignores_trailing_operands_and_invokes_the_private_installed_engine_once() {
    let home = std::env::temp_dir().join(format!("posture-heartbeat-edge-{}", std::process::id()));
    std::fs::create_dir(&home).unwrap();
    let engine = home.join(".cargo/bin/pns");
    std::fs::create_dir_all(engine.parent().unwrap()).unwrap();
    std::fs::write(&engine,br##"#!/bin/sh
set -eu
[ "$#" = 2 ] && [ "$1" = submit ] && [ "$2" = --json ] || exit 42
IFS= read -r request
printf '%s\n' "$request" >"$HOME/request"
printf 'call\n' >>"$HOME/calls"
identity="$(printf '%s' "$request" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')"
printf '{"schema":"pns.result/1","request_id":"%s","status":"accepted","diagnostics":["ledger_committed"]}\n' "$identity"
"##).unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o700)).unwrap();
    let log = home.join(".local/log/osquery/osqueryd.snapshots.log");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 17;
    let before = format!("{{\"name\":\"heartbeat_canary\",\"unixTime\":{epoch}}}\n");
    std::fs::write(&log, &before).unwrap();
    let ignored_override = home.join("ignored-canary");
    let ignored_before = format!(
        "{{\"name\":\"heartbeat_canary\",\"unixTime\":{}}}\n",
        epoch + 17
    );
    std::fs::write(&ignored_override, &ignored_before).unwrap();
    let deadline = Instant::now() + Duration::from_millis(650);
    let mut child = Command::new(env!("CARGO_BIN_EXE_posture"))
        .env_clear()
        .env("HOME", &home)
        .env("TMPDIR", &home)
        .env("TMP", &home)
        .env("TEMP", &home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_RUNTIME_DIR", home.join("runtime"))
        .env("CLAUDE_CONFIG_DIR", home.join("claude"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("OSQUERY_SNAPSHOTS_LOG", &ignored_override)
        .env("OSQUERY_CANARY_MAX_AGE", "020")
        .args([
            OsString::from("heartbeat"),
            "ignored".into(),
            "two words".into(),
            OsString::from_vec(vec![0xff]),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(1)),
            state => {
                let killed = child.kill();
                let reaped = child.wait();
                panic!("private heartbeat exceeded its bound: {state:?}, {killed:?}, {reaped:?}");
            }
        }
    }
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
    assert_eq!(std::fs::read(home.join("calls")).unwrap(), b"call\n");
    let request = std::fs::read_to_string(home.join("request")).unwrap();
    for text in [
        "\"event\":\"heartbeat\"",
        "\"kind\":\"observation\"",
        "\"route\":\"posture\"",
        "STALE",
        "over 020s",
    ] {
        assert!(request.contains(text), "{text}: {request}");
    }
    assert!(!request.contains("\"class\""));
    assert_eq!(std::fs::read_to_string(log).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(ignored_override).unwrap(),
        ignored_before
    );
    assert!(!home.join(".local/state").exists());
}
