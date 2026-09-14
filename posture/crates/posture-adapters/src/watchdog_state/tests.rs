use super::*;
use crate::test_sandbox::Sandbox;
use posture_domain::{Agent, AuditMemory, QueueMemory};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
/// A state-file path in a directory that removes itself. Hold the sandbox for
/// as long as the path is used.
fn fresh_path() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new("watchdog-state");
    let path = sandbox.path().join("state.json");
    (sandbox, path)
}
#[test]
fn legacy_state_migrates_without_losing_growth_or_confirmed_audit() {
    let (_sandbox, path) = fresh_path();
    let hash = "a".repeat(64);
    fs::write(&path, format!(r#"{{"agents":{{"{}":{{"runs":8,"streak":1}}}},"pending":{{"count":7,"growth_streak":1}},"pipeline_audit":{{"fingerprint":"{hash}","streak":999,"paged_fingerprint":"{hash}"}}}}"#,Agent::Digest.label())).unwrap();
    let state = WatchdogStateFile::new(path).load();
    assert_eq!(state.agents[3].unwrap().runs, Some(8));
    assert_eq!(
        state.legacy_pending,
        QueueMemory {
            count: Some(7),
            growth_streak: 1,
            // The legacy file predates the dead-letter baseline, so the first
            // tick after the upgrade observes the standing count afresh.
            deadletters: None
        }
    );
    assert_eq!(state.pns_pending, QueueMemory::default());
    assert_eq!(
        state.pipeline_audit,
        AuditMemory::from_readings(&hash, "99", &hash)
    );
}
#[test]
fn only_one_complete_top_level_object_can_supply_state() {
    for input in [
        "{}{}",
        "{} []",
        "[]",
        "null",
        "{",
        r#"{"pending":{"count":4}} {}"#,
    ] {
        let (_sandbox, path) = fresh_path();
        fs::write(&path, input).unwrap();
        assert_eq!(
            WatchdogStateFile::new(path).load(),
            WatchdogState::default()
        );
    }
}
#[test]
fn publication_is_private_and_round_trips_separate_growth_histories() {
    let (_sandbox, path) = fresh_path();
    let mut store = WatchdogStateFile::new(path.clone());
    assert!(store.writable());
    assert!(!path.exists());
    let state = WatchdogState {
        legacy_pending: QueueMemory {
            count: Some(9),
            growth_streak: 2,
            deadletters: Some(4),
        },
        pns_pending: QueueMemory {
            count: Some(5),
            growth_streak: 1,
            deadletters: Some(0),
        },
        ..WatchdogState::default()
    };
    store.publish(&state).unwrap();
    assert_eq!(store.load(), state);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}
#[test]
fn failed_publication_and_symlinked_input_never_damage_the_referent() {
    let (_sandbox, path) = fresh_path();
    let target = path.with_extension("target");
    fs::write(&target, b"private").unwrap();
    symlink(&target, &path).unwrap();
    let mut store = WatchdogStateFile::new(path.clone());
    assert_eq!(store.load(), WatchdogState::default());
    store.publish(&WatchdogState::default()).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"private");
    assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    let (_sandbox, path) = fresh_path();
    fs::create_dir(&path).unwrap();
    assert_eq!(
        WatchdogStateFile::new(path.clone()).publish(&WatchdogState::default()),
        Err(WatchdogStateFailure)
    );
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}
