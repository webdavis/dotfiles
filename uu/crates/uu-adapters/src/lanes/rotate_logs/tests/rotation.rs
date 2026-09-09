use super::fixtures::*;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[test]
fn a_file_of_exactly_the_threshold_rotates_and_one_byte_under_does_not() {
    for threshold in [1, 16] {
        let root = sandbox();
        let exact = root.join("exact.log");
        let under = root.join("under.log");
        let above = root.join("above.log");
        let empty = root.join("empty.log");
        fs::write(&exact, vec![1; threshold]).unwrap();
        fs::write(&under, vec![2; threshold - 1]).unwrap();
        fs::write(&above, vec![3; threshold + 1]).unwrap();
        fs::write(&empty, []).unwrap();
        let inode = fs::metadata(&under).unwrap().ino();
        let mut config = lane(&[&exact, &under, &above, &empty]);
        config.rotate_at_bytes = threshold as u64;
        let report = run(&config);
        assert_eq!(report.failures(), 0, "{:?}", report.lines);
        assert!(bytes(&exact).is_empty());
        assert_eq!(bytes(&under), vec![2; threshold - 1]);
        assert_eq!(fs::metadata(&under).unwrap().ino(), inode);
        assert!(bytes(&above).is_empty());
        assert!(bytes(&empty).is_empty());
        assert!(!archive(&under, 1).exists());
        assert!(!archive(&empty, 1).exists());
        assert!(
            report
                .lines
                .iter()
                .any(|l| l.contains("under threshold: 2")),
            "{:?}",
            report.lines
        );
    }
}
#[test]
fn an_oversized_log_is_archived_and_truncated_in_place_keeping_its_inode() {
    let root = sandbox();
    fs::create_dir(root.join("nested")).unwrap();
    let log = root.join("nested/service.log");
    let original: Vec<u8> = (0..200_000).map(|i| (i % 256) as u8).collect();
    fs::write(&log, &original).unwrap();
    let inode = fs::metadata(&log).unwrap().ino();
    let mut writer = fs::OpenOptions::new().append(true).open(&log).unwrap();
    let report = run(&lane(&[&log]));
    assert_eq!(report.name, "named-rotation");
    assert_eq!(report.failures(), 0, "{:?}", report.lines);
    assert!(bytes(&log).is_empty());
    assert_eq!(fs::metadata(&log).unwrap().ino(), inode);
    assert_eq!(inflate(&archive(&log, 1)), original);
    assert_eq!(
        fs::metadata(archive(&log, 1)).unwrap().permissions().mode() & 0o777,
        0o600
    );
    writer.write_all(b"still held").unwrap();
    assert_eq!(bytes(&log), b"still held");
    assert!(
        report.lines.iter().any(|l| l.contains("200000")),
        "{:?}",
        report.lines
    );
}
#[test]
fn retention_keeps_exactly_the_window_pruning_index_zero_and_a_lowered_keep_counts_strays() {
    let root = sandbox();
    let log = root.join("busy.log");
    fs::write(&log, [9; 16]).unwrap();
    for i in 0..=5 {
        fs::write(archive(&log, i), format!("old-{i}")).unwrap();
    }
    let foreign = root.join("busy.log.stray.gz");
    fs::write(&foreign, b"foreign").unwrap();
    assert_eq!(run(&lane(&[&log])).failures(), 0);
    assert_eq!(inflate(&archive(&log, 1)), [9; 16]);
    assert_eq!(bytes(&archive(&log, 2)), b"old-1");
    assert_eq!(bytes(&archive(&log, 3)), b"old-2");
    let mut names: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.to_str().unwrap().ends_with(".gz"))
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "busy.log.1.gz",
            "busy.log.2.gz",
            "busy.log.3.gz",
            "busy.log.stray.gz"
        ]
        .map(std::ffi::OsString::from)
    );
    fs::write(&log, [8; 16]).unwrap();
    let mut one = lane(&[&log]);
    one.archives_kept = 1;
    assert_eq!(run(&one).failures(), 0);
    assert!(!archive(&log, 2).exists());
    assert!(!archive(&log, 3).exists());
    assert_eq!(bytes(&foreign), b"foreign");
}
#[test]
fn a_log_the_list_does_not_name_is_never_touched() {
    let root = sandbox();
    let listed = root.join("listed.log");
    let other = root.join("other.log");
    fs::write(&listed, [1; 16]).unwrap();
    fs::write(&other, [2; 32]).unwrap();
    let before = fs::metadata(&other).unwrap();
    assert_eq!(run(&lane(&[&listed])).failures(), 0);
    assert!(bytes(&listed).is_empty());
    assert_eq!(bytes(&other), [2; 32]);
    assert_eq!(fs::metadata(&other).unwrap().ino(), before.ino());
    assert!(!archive(&other, 1).exists());
}
