//! The claims here were read off `results-alerter/digest-store.sh`, and the
//! round trip is checked against the reader that actually consumes the file.

use super::*;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicUsize, Ordering};

fn store() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "posture-append-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    root.join("digest.ndjson")
}

fn record(detector: &str) -> posture_protocol::DigestRecord {
    posture_protocol::DigestRecord {
        timestamp: Some("2026-09-09T12:00:00Z".into()),
        detector: Some(detector.into()),
        category: Some("launchd".into()),
        identity: Some("com.example.agent".into()),
        action: Some("added".into()),
        summary: Some("an agent appeared".into()),
    }
}

#[test]
fn a_record_survives_the_round_trip_to_the_reader_that_consumes_it() {
    // THE TWO ENDS NEVER CALL EACH OTHER. What keeps them agreeing is that both
    // build the line from one crate, so this asserts against the decoder rather
    // than against a string this test wrote.
    let store = store();
    let appender = DigestAppendFile::new(store.clone());
    assert!(appender.append(&record("persistence_launchd")));
    let read = posture_protocol::decode_spool(&std::fs::read_to_string(&store).unwrap());
    assert_eq!(read, vec![record("persistence_launchd")]);
}

#[test]
fn every_appended_record_is_kept_and_none_replaces_another() {
    // A day's spool is many findings, and an append that truncated would leave
    // the digest reporting the last one as the whole day.
    let store = store();
    let appender = DigestAppendFile::new(store.clone());
    for detector in ["new_admin_user", "suid_bin_unexpected", "recent_logins"] {
        assert!(appender.append(&record(detector)));
    }
    let read = posture_protocol::decode_spool(&std::fs::read_to_string(&store).unwrap());
    assert_eq!(
        read.iter()
            .map(|r| r.detector.as_deref().unwrap_or(""))
            .collect::<Vec<_>>(),
        ["new_admin_user", "suid_bin_unexpected", "recent_logins"]
    );
}

#[test]
fn each_record_is_one_whole_line_so_a_reader_can_split_on_newlines() {
    let store = store();
    let appender = DigestAppendFile::new(store.clone());
    appender.append(&record("a"));
    appender.append(&record("b"));
    let text = std::fs::read_to_string(&store).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.ends_with('\n'), "a torn last line is a dropped record");
    assert!(!text.contains("\n\n"));
}

#[test]
fn the_spool_and_its_directory_are_readable_by_nobody_else() {
    // IT HOLDS FULL FILESYSTEM PATHS, and it outlives the alert by a day.
    let store = store();
    assert!(DigestAppendFile::new(store.clone()).append(&record("a")));
    assert_eq!(
        std::fs::metadata(&store).unwrap().mode() & 0o777,
        0o600,
        "the spool"
    );
    assert_eq!(
        std::fs::metadata(store.parent().unwrap()).unwrap().mode() & 0o777,
        0o700,
        "its directory"
    );
}

#[test]
fn a_spool_something_else_loosened_is_tightened_rather_than_left_open() {
    // `mode` on the open says nothing about a file that already exists, so a
    // spool that was made world-readable would stay that way for its whole life.
    let store = store();
    let appender = DigestAppendFile::new(store.clone());
    appender.append(&record("a"));
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o644)).unwrap();
    appender.append(&record("b"));
    assert_eq!(std::fs::metadata(&store).unwrap().mode() & 0o777, 0o600);
}

#[test]
fn a_spool_that_cannot_be_written_is_reported_rather_than_claimed() {
    // A SPOOL FAILURE IS NOT A PAGE FAILURE, but it is not a success either:
    // the caller is told, and what it does with that is the caller's rule.
    let blocked = std::env::temp_dir().join(format!("posture-append-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&blocked);
    std::fs::write(&blocked, b"not a directory\n").unwrap();
    let appender = DigestAppendFile::new(blocked.join("digest.ndjson"));
    assert!(!appender.append(&record("a")));
    let _ = std::fs::remove_file(&blocked);
}

#[test]
fn appending_into_a_directory_that_does_not_exist_yet_makes_it_first() {
    // The alerter can be the first thing on a fresh machine to write here.
    let store = store();
    assert!(!store.parent().unwrap().exists());
    assert!(DigestAppendFile::new(store.clone()).append(&record("a")));
    assert!(store.exists());
}
