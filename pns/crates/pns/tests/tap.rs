#[path = "tap/edges.rs"]
mod edges;
mod support;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, SystemTime};
use support::{Sandbox, run, stdout};

fn tap(sandbox: &Sandbox, words: &[&str]) -> std::process::Output {
    sandbox.pns().args(words).output().expect("run tap")
}

fn json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("one JSON result")
}

#[test]
fn tap_without_config_creates_the_default_marker_and_reports_mobile() {
    let s = Sandbox::without_config("tap-fresh-home");
    let out = tap(&s, &["tap", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let answer = json(&out);
    assert_eq!(answer["schema"], "pns.tap/1");
    assert_eq!(answer["write_status"], "recorded");
    assert_eq!(answer["surface"], "mobile");
    assert_eq!(answer["marker"]["source"], "default");
    assert!(s.path(".local/state/pns/phone-attention.marker").is_file());
    for created in [".local", ".local/state", ".local/state/pns"] {
        let mode = fs::metadata(s.path(created)).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{created} is not private: {mode:o}");
    }
    for channel in ["mobile", "hermes", "banner"] {
        assert!(!s.fired(channel));
    }
}

#[test]
fn the_json_marker_dates_the_recorded_tap() {
    let s = Sandbox::without_config("tap-touched-at");
    let answer = json(&tap(&s, &["tap", "--json"]));
    let recorded = &answer["marker"];
    let mtime = recorded["mtime_epoch_secs"]
        .as_u64()
        .expect("a recorded mtime");
    assert_eq!(
        recorded["touched_at"].as_str(),
        pns_adapters::utc_timestamp(mtime).as_deref(),
        "{recorded}"
    );
    let never = Sandbox::without_config("tap-never-touched");
    let absent = json(&tap(&never, &["tap", "info", "--json"]));
    assert_eq!(absent["marker"]["exists"], false);
    assert!(absent["marker"]["touched_at"].is_null(), "{absent}");
}

#[test]
fn tap_and_the_event_reader_share_the_configured_marker() {
    let s = Sandbox::new("tap-shared-config");
    let path = s.path("custom/attention");
    s.write_config(&format!(
        "{}\n[phone]\nmarker_file = {:?}\n",
        support::STUB_CHANNELS,
        path
    ));
    let out = tap(&s, &["tap", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(json(&out)["marker"]["source"], "config");
    assert!(path.is_file());
    run(s.pns().env("PNS_SCREEN_IDLE", "60").args([
        "send",
        "--producer",
        "shell",
        "--state",
        "done",
        "--detail",
        "tap",
    ]));
    assert!(s.fired("mobile"));
    assert!(!s.fired("banner"));
    assert!(!s.path(".local/state/pns/phone-attention.marker").exists());
}

/// THE MUTANT THIS PINS: `PNS_PHONE_MARKER_FILE` read back in, which would
/// have this run silently succeed off the environment path even with a
/// config that could not load. Config is the only source now, so a broken
/// config refuses instead, and the deleted variable is never touched.
#[test]
fn the_marker_environment_override_is_gone_a_broken_config_refuses_instead() {
    let s = Sandbox::new("tap-env-ignored");
    s.write_config("[broken");
    let marker = s.path("override");
    let out = s
        .pns()
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["tap", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(json(&out)["error"]["code"], "config_error");
    assert!(!marker.exists(), "the deleted override must not be touched");
}

#[test]
fn info_preserves_missing_state_and_install_preserves_existing_state() {
    let s = Sandbox::without_config("tap-read-only");
    let out = tap(&s, &["tap", "info", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(json(&out)["marker"]["exists"], false);
    assert!(json(&out)["marker"]["age_secs"].is_null());
    assert!(!s.path(".local/state/pns").exists());
    let marker = s.path("marker");
    fs::write(&marker, "keep").unwrap();
    let before = fs::metadata(&marker).unwrap().modified().unwrap();
    s.write_config(&format!("[phone]\nmarker_file = {marker:?}\n"));
    let out = s.pns().args(["tap", "install", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let answer = json(&out);
    assert_eq!(answer["write_status"], "not_requested");
    assert!(answer["install"]["shortcut_url"].is_null());
    assert!(answer["install"]["verified_ios"].is_null());
    assert_eq!(answer["install"]["steps"].as_array().unwrap().len(), 3);
    assert!(
        answer["install"]["authorized_key_line"]
            .as_str()
            .unwrap()
            .contains(" tap")
    );
    assert_eq!(fs::read_to_string(&marker).unwrap(), "keep");
    assert_eq!(fs::metadata(marker).unwrap().modified().unwrap(), before);
    assert!(!s.path(".ssh").exists());
}

#[test]
fn a_failed_directory_creation_is_an_operational_failure() {
    let s = Sandbox::without_config("tap-mkdir-fails");
    fs::write(s.path("blocked"), "keep").unwrap();
    s.write_config(&format!(
        "[phone]\nmarker_file = {:?}\n",
        s.path("blocked/marker")
    ));
    let out = s.pns().args(["tap", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(json(&out)["ok"], false);
    assert_eq!(json(&out)["error"]["code"], "mkdir_failed");
    let reported = json(&out)["error"]["message"].as_str().unwrap().to_owned();
    assert!(reported.contains("os error 17"), "{reported}");
    assert_eq!(fs::read_to_string(s.path("blocked")).unwrap(), "keep");
}

#[test]
fn a_directory_at_the_marker_is_refused_without_claiming_success() {
    let s = Sandbox::without_config("tap-directory-marker");
    s.write_config(&format!("[phone]\nmarker_file = {:?}\n", s.root));
    let out = s.pns().args(["tap", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(json(&out)["write_status"], "failed");
    assert_eq!(json(&out)["error"]["code"], "touch_failed");
}

#[test]
fn tap_preserves_contents_and_desk_wins_a_tie() {
    let s = Sandbox::without_config("tap-desk-tie");
    let marker = s.path("marker");
    fs::write(&marker, "opaque contents").unwrap();
    fs::File::open(&marker)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1))
        .unwrap();
    s.write_config(&format!("[phone]\nmarker_file = {marker:?}\n"));
    let out = s
        .pns()
        .env("PNS_SCREEN_IDLE", "0")
        .args(["tap", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(json(&out)["surface"], "desk");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "opaque contents");
    assert!(
        fs::metadata(marker).unwrap().modified().unwrap()
            > SystemTime::UNIX_EPOCH + Duration::from_secs(1)
    );
}

#[test]
fn invalid_config_refuses_a_tap_without_falling_back_or_exposing_values() {
    let s = Sandbox::new("tap-invalid-config");
    s.write_config("[phone]\nmarker_file = 42\n");
    let out = tap(&s, &["tap", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(json(&out)["error"]["code"], "config_error");
    assert!(json(&out)["marker"].is_null(), "{out:?}");
    assert!(!stdout(&out).contains("42"));
    assert!(!s.path(".local/state/pns").exists());
}

#[test]
fn forbidden_flags_and_conflicting_operations_fail_before_touch() {
    let s = Sandbox::without_config("tap-invalid-argv");
    for words in [
        vec!["tap", "--clear", "--json"],
        vec!["tap", "--for", "5m", "--json"],
        vec!["tap", "--set-marker", "/tmp/no", "--json"],
        vec!["tap", "info", "install", "--json"],
    ] {
        let out = tap(&s, &words);
        assert_eq!(out.status.code(), Some(2), "{out:?}");
        assert_eq!(json(&out)["error"]["code"], "invalid_arguments");
    }
    assert!(!s.path(".local/state/pns").exists());
}

#[test]
fn the_retired_flag_spelling_is_refused_and_names_the_subcommand() {
    let s = Sandbox::without_config("tap-retired-flags");
    for (words, verb) in [
        (vec!["tap", "--install", "--json"], "pns tap install"),
        (vec!["tap", "--info", "--json"], "pns tap info"),
    ] {
        let out = tap(&s, &words);
        assert_eq!(out.status.code(), Some(2), "{out:?}");
        let answer = json(&out);
        assert_eq!(answer["error"]["code"], "invalid_arguments");
        let message = answer["error"]["message"].as_str().expect("a message");
        assert!(message.contains(verb), "{message}");
    }
    assert!(!s.path(".local/state/pns").exists());
}

#[test]
fn the_guide_uses_the_shared_style_and_discloses_unverified_phone_setup() {
    let s = Sandbox::without_config("tap-guide");
    let out = tap(&s, &["--no-color", "tap", "install"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let text = stdout(&out);
    assert!(text.starts_with("\npns tap install\n"), "{text}");
    for text_part in [
        "◆ 1. This Mac",
        "◆ 2. Your phone",
        "◆ 3. Trigger methods",
        "Remote Login",
        "not included",
        "pns tap info",
        "restrict",
        "Port",
        "Undo",
    ] {
        assert!(text.contains(text_part), "missing {text_part}: {text}");
    }
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains("icloud.com/shortcuts/"));
}

#[test]
fn tap_updates_a_dangling_link_itself_without_creating_its_target() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let s = Sandbox::without_config("tap-dangling-link");
    let marker = s.path("link");
    let target = s.path("absent-target");
    std::os::unix::fs::symlink(&target, &marker).unwrap();
    let name = CString::new(marker.as_os_str().as_bytes()).unwrap();
    let times = [libc::timespec {
        tv_sec: 1,
        tv_nsec: 0,
    }; 2];
    // SAFETY: name is terminated and times supplies both required timestamps.
    assert_eq!(
        unsafe {
            libc::utimensat(
                libc::AT_FDCWD,
                name.as_ptr(),
                times.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        },
        0
    );
    s.write_config(&format!("[phone]\nmarker_file = {marker:?}\n"));
    let out = s.pns().args(["tap", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(json(&out)["marker"]["mtime_epoch_secs"].as_u64().unwrap() > 1);
    assert!(
        fs::symlink_metadata(&marker)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!target.exists());
}

#[test]
fn doctor_reports_the_configured_tap_as_missing_fresh_or_stale_without_writing_it() {
    let s = Sandbox::new("doctor-tap-marker");
    let path = s.path("attention");
    s.write_config(&format!(
        "{}\n[phone]\nmarker_file = {:?}\n",
        support::STUB_CHANNELS,
        path
    ));
    for state in ["never tapped", "fresh", "stale"] {
        if state != "never tapped" {
            fs::write(&path, "private contents").unwrap();
            if state == "stale" {
                fs::File::open(&path)
                    .unwrap()
                    .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1))
                    .unwrap();
            }
        }
        let before = fs::metadata(&path).ok().map(|m| m.modified().unwrap());
        let out = s
            .pns()
            .env("PNS_MOSHI_HOOK_BIN", s.path("absent-moshi-hook"))
            .args(["doctor", "--no-color"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        let printed = stdout(&out);
        let line = printed
            .lines()
            .find(|line| line.contains("phone tap:"))
            .expect("marker row");
        assert!(
            line.contains(state) && line.contains("config") && line.contains("attention"),
            "{line}"
        );
        assert!(line.contains("pns tap info"));
        assert!(!printed.contains("private contents"));
        assert_eq!(
            fs::metadata(&path).ok().map(|m| m.modified().unwrap()),
            before
        );
    }
}
