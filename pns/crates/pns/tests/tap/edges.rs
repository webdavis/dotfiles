use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn a_denied_marker_write_is_nonzero_and_preserves_existing_state() {
    let s = Sandbox::without_config("tap-permission-denied");
    let parent = s.path("private");
    fs::create_dir(&parent).unwrap();
    let marker = parent.join("marker");
    fs::write(&marker, "keep").unwrap();
    let before = fs::metadata(&marker).unwrap().modified().unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o000)).unwrap();
    let outcome = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["tap", "--json"])
        .output();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let out = outcome.unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(json(&out)["write_status"], "failed");
    assert_eq!(json(&out)["error"]["code"], "touch_failed");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "keep");
    assert_eq!(fs::metadata(&marker).unwrap().modified().unwrap(), before);
}

#[test]
fn remote_login_leads_the_mac_steps_and_info_says_what_the_phone_shows() {
    let s = Sandbox::without_config("tap-remote-login");
    let guide = stdout(&tap(&s, &["--no-color", "tap", "--install"]));
    let lines: Vec<&str> = guide.lines().collect();
    let step = lines
        .iter()
        .position(|line| line.contains("1. This Mac") && line.contains('◆'))
        .expect("the first Mac step");
    let first = lines.get(step + 1).expect("a line under the first step");
    for expected in ["Remote Login", "System Settings", "General", "Sharing"] {
        assert!(first.contains(expected), "missing {expected}: {first}");
    }
    let info = stdout(&tap(&s, &["--no-color", "tap", "--info"]));
    let cannot_answer = info
        .lines()
        .find(|line| line.contains("SSH"))
        .expect("a line about a Mac that cannot answer");
    assert!(cannot_answer.contains("notification"), "{cannot_answer}");
    assert!(info.contains("Remote Login"), "{info}");
    assert!(info.contains("System Settings"), "{info}");
}

#[test]
fn a_failed_tap_reports_the_marker_path_and_the_reason_on_one_stderr_line() {
    let s = Sandbox::without_config("tap-failure-line");
    let parent = s.path("private");
    fs::create_dir(&parent).unwrap();
    let marker = parent.join("marker");
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o000)).unwrap();
    let outcome = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["tap"])
        .output();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let out = outcome.unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(stdout(&out).is_empty(), "{out:?}");
    let reported = support::stderr(&out);
    assert_eq!(reported.trim_end().lines().count(), 1, "{reported}");
    assert!(
        reported.contains(marker.to_str().unwrap()),
        "the path is unnamed: {reported}"
    );
    assert!(reported.contains("permission denied"), "{reported}");
}

#[test]
fn empty_environment_uses_the_tilde_config_path_with_private_creation_modes() {
    let s = Sandbox::new("tap-tilde");
    s.write_config("[phone]\nmarker_file = '~/attention/marker'");
    let out = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", "")
        .args(["tap", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        json(&out)["marker"]["path"],
        s.path("attention/marker").to_str().unwrap()
    );
    assert_eq!(
        fs::metadata(s.path("attention"))
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert_eq!(
        fs::metadata(s.path("attention/marker"))
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
}

#[test]
fn metadata_errors_remain_unknown_in_info_and_doctor() {
    let s = Sandbox::new("tap-metadata-error");
    fs::write(s.path("obstacle"), "private contents").unwrap();
    let marker = s.path("obstacle/marker");
    let out = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["tap", "--info", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let answer = json(&out);
    assert_eq!(answer["error"]["code"], "marker_unreadable");
    for field in ["exists", "mtime_epoch_secs", "age_secs", "fresh"] {
        assert!(answer["marker"][field].is_null(), "{answer}");
    }
    assert_eq!(answer["write_status"], "not_requested");
    let plain = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["tap", "--info", "--no-color"])
        .output()
        .unwrap();
    let reported = support::stderr(&plain);
    for expected in [
        "obstacle/marker",
        "PNS_PHONE_MARKER_FILE",
        "Last tap: unknown",
        "asleep",
        "retry",
    ] {
        assert!(
            reported.contains(expected),
            "missing {expected}: {reported}"
        );
    }
    let out = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .env("MOSHI_HOOK_BIN", s.path("absent-hook"))
        .args(["doctor", "--no-color"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(stdout(&out).contains("phone tap: unknown"));
    assert!(!stdout(&out).contains("private contents"));
}

#[test]
fn concurrent_taps_on_a_link_preserve_its_target_and_existing_permissions() {
    let s = Sandbox::without_config("tap-concurrent");
    let target = s.path("target");
    fs::write(&target, "opaque contents").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    fs::File::open(&target)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1))
        .unwrap();
    let before = fs::metadata(&target).unwrap();
    let marker = s.path("link");
    symlink(&target, &marker).unwrap();
    let children: Vec<_> = (0..4)
        .map(|_| {
            s.pns()
                .env("PNS_PHONE_MARKER_FILE", &marker)
                .args(["tap", "--json"])
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(json(&out)["write_status"], "recorded");
    }
    let after = fs::metadata(&target).unwrap();
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(before.permissions(), after.permissions());
    assert_eq!(fs::read_to_string(&target).unwrap(), "opaque contents");
    assert!(
        fs::symlink_metadata(marker)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn future_marker_is_fresh_but_an_invalid_window_is_unknown() {
    let s = Sandbox::without_config("tap-future");
    let marker = s.path("marker");
    fs::write(&marker, "").unwrap();
    fs::File::open(&marker)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(3600))
        .unwrap();
    for window in ["120", "invalid"] {
        let out = s
            .pns()
            .env("PNS_PHONE_MARKER_FILE", &marker)
            .env("PNS_DESK_IDLE_SECS", window)
            .args(["tap", "--info", "--json"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        let answer = json(&out);
        assert_eq!(answer["marker"]["age_secs"], 0);
        assert_eq!(
            answer["marker"]["fresh"],
            if window == "120" {
                Value::Bool(true)
            } else {
                Value::Null
            }
        );
    }
}
