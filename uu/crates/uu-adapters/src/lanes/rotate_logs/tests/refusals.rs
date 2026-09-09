use super::fixtures::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn a_symlink_is_never_truncated_through() {
    let root = sandbox();
    let target = root.join("target.log");
    let link = root.join("link.log");
    let control = root.join("control.log");
    fs::write(&target, [7; 32]).unwrap();
    fs::write(&control, [1; 16]).unwrap();
    symlink(&target, &link).unwrap();
    let inode = fs::metadata(&target).unwrap().ino();
    assert_eq!(run(&lane(&[&link, &control])).failures(), 0);
    assert!(bytes(&control).is_empty());
    assert_eq!(bytes(&target), [7; 32]);
    assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert!(!archive(&link, 1).exists());
}
#[test]
fn an_unwritable_log_over_the_threshold_is_a_failure_naming_it() {
    let root = sandbox();
    let locked = root.join("locked.log");
    let small = root.join("small.log");
    let control = root.join("control.log");
    fs::write(&locked, [4; 32]).unwrap();
    fs::write(&small, [5; 15]).unwrap();
    fs::write(&control, [6; 16]).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o400)).unwrap();
    fs::set_permissions(&small, fs::Permissions::from_mode(0o400)).unwrap();
    let report = run(&lane(&[&locked, &small, &control]));
    assert_eq!(report.failures(), 1, "{:?}", report.lines);
    assert!(report.last_failure().unwrap().contains("locked.log"));
    assert!(!report.lines.iter().any(|l| l.contains("small.log")));
    assert_eq!(bytes(&locked), [4; 32]);
    assert_eq!(bytes(&small), [5; 15]);
    assert!(bytes(&control).is_empty());
}
#[test]
fn an_archive_of_our_own_is_never_re_archived() {
    let root = sandbox();
    let archived = root.join("old.log.1.gz");
    let control = root.join("control.log");
    fs::write(&archived, [8; 32]).unwrap();
    fs::write(&control, [7; 16]).unwrap();
    let inode = fs::metadata(&archived).unwrap().ino();
    let report = run(&lane(&[&archived, &control]));
    assert_eq!(report.failures(), 0);
    assert!(bytes(&control).is_empty());
    assert_eq!(bytes(&archived), [8; 32]);
    assert_eq!(fs::metadata(&archived).unwrap().ino(), inode);
    assert!(!archive(&archived, 1).exists());
    assert!(!report.lines.iter().any(|l| l.contains("old.log.1.gz")));
}
#[test]
fn a_compress_that_fails_leaves_the_log_untruncated_and_no_partial_behind() {
    let root = sandbox();
    let log = root.join("busy.log");
    fs::write(&log, [1; 32]).unwrap();
    let mut config = lane(&[&log]);
    let compressor = root.join("fail-compressor");
    fs::write(&compressor, "#!/bin/sh\nprintf partial\nexit 7\n").unwrap();
    fs::set_permissions(&compressor, fs::Permissions::from_mode(0o700)).unwrap();
    config.compressor = compressor.to_str().unwrap().into();
    let report = run(&config);
    assert_eq!(report.failures(), 1);
    assert!(report.last_failure().unwrap().contains("busy.log"));
    assert!(report.last_failure().unwrap().contains("exit 7"));
    assert_eq!(bytes(&log), [1; 32]);
    assert!(!archive(&log, 1).exists());
    assert!(!root.join("busy.log.1.gz.partial").exists());
}
#[test]
fn an_existing_partial_symlink_cannot_redirect_compression() {
    let root = sandbox();
    let log = root.join("busy.log");
    let outside = root.join("outside");
    fs::write(&log, [1; 32]).unwrap();
    fs::write(&outside, b"sentinel").unwrap();
    symlink(&outside, root.join("busy.log.1.gz.partial")).unwrap();
    assert_eq!(run(&lane(&[&log])).failures(), 1);
    assert_eq!(bytes(&outside), b"sentinel");
    assert_eq!(bytes(&log), [1; 32]);
}

#[test]
fn a_successful_compressor_with_no_archive_bytes_never_truncates_the_log() {
    let root = sandbox();
    let log = root.join("busy.log");
    fs::write(&log, [1; 32]).unwrap();
    let mut config = lane(&[&log]);
    config.compressor = "/usr/bin/true".into();
    let report = run(&config);
    assert_eq!(report.failures(), 1);
    assert!(report.last_failure().unwrap().contains("empty"));
    assert_eq!(bytes(&log), [1; 32]);
    assert!(!archive(&log, 1).exists());
    assert!(!root.join("busy.log.1.gz.partial").exists());
}
#[test]
fn an_unreadable_size_is_named_and_does_not_stop_the_next_log() {
    let root = sandbox();
    let private = root.join("unsearchable");
    fs::create_dir(&private).unwrap();
    let log = private.join("unreadable.log");
    let control = root.join("control.log");
    fs::write(&log, [1; 32]).unwrap();
    fs::write(&control, [2; 16]).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
    let report = run(&lane(&[&log, &control]));
    assert_eq!(report.failures(), 1);
    assert!(report.last_failure().unwrap().contains("unreadable.log"));
    assert!(bytes(&control).is_empty());
}
