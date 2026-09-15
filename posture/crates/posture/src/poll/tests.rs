use super::*;
mod fixture;
use fixture::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn query_lookup_skips_non_executable_files_and_keeps_the_absolute_fallback() {
    let subject = Subject::new();
    let root = subject.controls().parent().unwrap().to_path_buf();
    let shadow = root.join("shadow");
    let bin = root.join("bin");
    for (directory, mode) in [(&shadow, 0o600), (&bin, 0o700)] {
        fs::create_dir(directory).unwrap();
        let path = directory.join("osqueryi");
        fs::write(&path, b"unused fixture").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    let paths = std::env::join_paths([shadow, bin.clone()]).unwrap();
    assert_eq!(query_path(Some(&paths)), bin.join("osqueryi"));
    assert_eq!(query_path(None), PathBuf::from("/usr/local/bin/osqueryi"));
}

#[test]
fn query_lookup_skips_files_executable_only_by_other_users() {
    let subject = Subject::new();
    let root = subject.controls().parent().unwrap().to_path_buf();
    let shadow = root.join("shadow");
    let bin = root.join("bin");
    for (directory, mode) in [(&shadow, 0o601), (&bin, 0o700)] {
        fs::create_dir(directory).unwrap();
        let path = directory.join("osqueryi");
        fs::write(&path, b"unused fixture").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    let paths = std::env::join_paths([shadow, bin.clone()]).unwrap();
    assert_eq!(query_path(Some(&paths)), bin.join("osqueryi"));
}

#[test]
fn healthy_reads_seed_a_private_baseline_and_the_next_tick_is_silent() {
    let subject = Subject::new();
    let (status, error, effects) = subject.run(HEALTHY, "FileVault is On.", true);
    assert_eq!(status, 0);
    assert!(error.is_empty(), "{error}");
    let effects = effects.borrow();
    assert_eq!(effects.commands, ["query", "control"]);
    assert!(effects.requests.is_empty());
    let state = fs::read_to_string(subject.state()).unwrap();
    assert!(state.contains("\"vault\":\"on\""), "{state}");
    assert_eq!(
        fs::metadata(subject.state()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let (_, error, effects) = subject.run(HEALTHY, "FileVault is On.", true);
    assert!(error.is_empty());
    assert!(effects.borrow().requests.is_empty());
    assert_eq!(fs::read_to_string(subject.state()).unwrap(), state);
}

#[test]
fn an_exposure_is_stored_before_the_baseline_changes_and_refusal_retains_it() {
    let subject = Subject::new();
    subject.seed();
    let before = fs::read(subject.state()).unwrap();
    let (status, error, effects) = subject.run(EXPOSED, "FileVault is On.", false);
    assert_eq!(status, 1);
    assert!(error.contains("baseline not advanced"), "{error}");
    assert_eq!(fs::read(subject.state()).unwrap(), before);
    let effects = effects.borrow();
    assert_eq!(effects.requests.len(), 1);
    assert_eq!(effects.baselines_at_submit, [Some(before.clone())]);
    assert!(effects.requests[0].contains("\"class\":\"security\""));
    assert!(effects.requests[0].contains("\"route\":\"posture-pages\""));
    assert!(effects.requests[0].contains("\"occurred_at\":10000"));
    assert_captured_detail(&effects.requests[0], "exposure:0");
    let (status, error, effects) = subject.run(EXPOSED, "FileVault is On.", true);
    assert_eq!(status, 0);
    assert!(error.is_empty());
    assert_eq!(effects.borrow().baselines_at_submit, [Some(before)]);
    assert!(
        fs::read_to_string(subject.state())
            .unwrap()
            .contains("\"firewall\":\"0\"")
    );
}

#[test]
fn refused_controls_never_run_probes_and_the_gap_precedes_the_exposure() {
    let subject = Subject::new();
    fs::write(subject.controls(), b"[]").unwrap();
    let (status, error, effects) = subject.run(EXPOSED, "must not run", true);
    assert_eq!(status, 0);
    assert!(error.is_empty(), "{error}");
    let effects = effects.borrow();
    assert_eq!(effects.commands, ["query", "submit", "submit"]);
    assert_eq!(effects.requests.len(), 2);
    assert!(effects.requests[0].contains("\"event\":\"gap\""));
    assert!(effects.requests[1].contains("\"event\":\"page\""));
    assert_captured_detail(&effects.requests[0], "refused-controls:0");
    assert_captured_detail(&effects.requests[1], "refused-controls:1");
    assert_eq!(effects.baselines_at_submit, [None, None]);
    assert_eq!(
        fs::read_to_string(subject.marker(".gap")).unwrap(),
        "controls_file\n"
    );
}

#[test]
fn unreadable_controls_preserve_the_open_error_and_still_submit_the_gap() {
    let subject = Subject::new();
    fs::set_permissions(subject.controls(), fs::Permissions::from_mode(0o000)).unwrap();
    let (status, error, effects) = subject.run(HEALTHY, "must not run", true);
    assert_eq!(status, 0);
    assert!(
        error.contains(".local/libexec/posture/controls.json")
            && error.contains("Permission denied"),
        "{error}"
    );
    assert_eq!(effects.borrow().commands, ["query", "submit"]);
    assert!(effects.borrow().requests[0].contains("not a JSON array"));
}

#[test]
fn stale_legacy_controls_cannot_hide_a_missing_relocated_file() {
    let subject = Subject::new();
    fs::create_dir_all(subject.legacy_controls().parent().unwrap()).unwrap();
    fs::rename(subject.controls(), subject.legacy_controls()).unwrap();
    let (status, error, effects) = subject.run(HEALTHY, "FileVault is On.", true);
    assert_eq!(status, 0);
    assert!(error.is_empty(), "{error}");
    let effects = effects.borrow();
    assert_eq!(effects.commands, ["query", "submit"]);
    assert_eq!(effects.requests.len(), 1);
    assert!(effects.requests[0].contains("posture-controls file missing at"));
    assert!(effects.requests[0].contains(".local/libexec/posture/controls.json"));
    assert_eq!(
        fs::read_to_string(subject.marker(".gap")).unwrap(),
        "controls_file\n"
    );
    assert!(subject.legacy_controls().is_file());
}

#[test]
fn unreadable_baseline_reports_the_open_error_without_trusting_or_stopping_it() {
    let subject = Subject::new();
    subject.seed();
    fs::set_permissions(subject.state(), fs::Permissions::from_mode(0o000)).unwrap();
    let (status, error, effects) = subject.run(HEALTHY, "FileVault is On.", true);
    assert_eq!(status, 0);
    assert!(
        error.contains("baseline.json") && error.contains("Permission denied"),
        "{error}"
    );
    assert!(effects.borrow().requests.is_empty());
    assert_eq!(
        fs::metadata(subject.state()).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn a_timed_out_query_keeps_the_trusted_trio_and_still_updates_clean_controls() {
    let subject = Subject::new();
    subject.seed();
    let (status, error, effects) = subject.run(QUERY_TIMEOUT, "FileVault is Off.", true);
    assert_eq!(status, 0);
    assert!(error.is_empty());
    let effects = effects.borrow();
    assert_eq!(effects.requests.len(), 2);
    assert!(effects.requests[0].contains("\"event\":\"gap\""));
    assert!(effects.requests[1].contains("\"event\":\"page\""));
    let state = fs::read_to_string(subject.state()).unwrap();
    assert!(state.contains("\"firewall\":\"1\""), "{state}");
    assert!(state.contains("\"vault\":\"off\""), "{state}");
    assert_eq!(
        fs::read_to_string(subject.marker(".gap")).unwrap(),
        "posture_query\n"
    );
}

#[test]
fn a_refused_gap_never_advances_the_marker_or_sends_an_exposure() {
    let subject = Subject::new();
    fs::write(subject.controls(), b"[]").unwrap();
    let (status, error, effects) = subject.run(EXPOSED, "must not run", false);
    assert_eq!(status, 1);
    assert_eq!(
        error,
        "firewall-gatekeeper-monitor: send_alert could not queue the monitoring-gap page; no marker written, retrying next tick\n"
    );
    assert_eq!(effects.borrow().requests.len(), 1);
    assert!(!subject.state().exists());
    assert!(!subject.marker(".gap").exists());
}

#[test]
fn a_failed_baseline_rename_is_not_hidden_by_an_accepted_persistence_gap() {
    let subject = Subject::new();
    fs::create_dir_all(subject.state()).unwrap();
    let (status, error, effects) = subject.run(HEALTHY, "FileVault is On.", true);
    assert_eq!(status, 1);
    assert!(error.is_empty());
    assert!(subject.state().is_dir());
    assert_eq!(effects.borrow().requests.len(), 1);
    assert!(effects.borrow().requests[0].contains("could not persist its baseline"));
    assert_eq!(
        fs::read_to_string(subject.marker(".persist-gap")).unwrap(),
        "baseline_persist\n"
    );
    assert!(!subject.marker(".tmp").exists());
}
