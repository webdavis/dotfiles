use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(format!(
        "/private/tmp/posture-triage-adapter-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

#[test]
fn actual_producer_matches_the_bash_capture_with_quotes_and_an_empty_version() {
    let root = scratch();
    let target = root.join(".local/bin/tool\"quoted");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, b"abc").unwrap();
    let managed = root.join("managed");
    std::fs::write(
        &managed,
        format!("{} 0600 1 {}\n", "A".repeat(64), target.display()),
    )
    .unwrap();
    let manifests = KnownGoodManifests::new(
        root.join("absent-pipeline"),
        managed,
        root.to_string_lossy().into_owned(),
    );
    let record = root.join("upgrade");
    std::fs::write(
        &record,
        "9000\t1970-01-01T02:30:00Z\ntool\"quoted\tadded\t\t2\"beta\n",
    )
    .unwrap();
    let mut diagnostics = Vec::new();
    let facts = file_integrity_triage(
        &manifests,
        &record,
        target.to_str().unwrap(),
        Some(10_000),
        &mut diagnostics,
    );
    let actual = serde_json::json!({"recorded":facts.recorded, "ondisk":facts.ondisk, "upgrade":facts.upgrade});
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("tests/fixtures/produced.json")).unwrap();
    assert_eq!(actual, expected);
    assert!(diagnostics.is_empty());
    assert!(
        !manifests.vouches(target.to_str().unwrap()),
        "display facts never grant trust"
    );
}

#[test]
fn disk_facts_distinguish_links_missing_files_and_nonregular_files() {
    let root = scratch();
    let absent = root.join("absent");
    let link = root.join("link");
    std::os::unix::fs::symlink(&absent, &link).unwrap();
    assert_eq!(disk_hash(&link), "a symbolic link");
    assert_eq!(disk_hash(&absent), "absent");
    assert_eq!(disk_hash(&root), "not a regular file");
    let regular = root.join("regular");
    std::fs::write(&regular, b"abc").unwrap();
    assert_eq!(disk_hash(&regular), "ba7816bf8f01");
}

#[test]
fn upgrade_reads_follow_regular_symlinks_but_refuse_pipes_devices_and_oversize_records() {
    let root = scratch();
    let record = root.join("record");
    let link = root.join("link");
    std::fs::write(&record, "9000\t1970-01-01T02:30:00Z\n").unwrap();
    std::os::unix::fs::symlink(&record, &link).unwrap();
    let mut diagnostics = Vec::new();
    assert!(
        upgrade_line(&link, "tool", Some(9000), &mut diagnostics)
            .ends_with("recorded no package change")
    );
    let fifo = root.join("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: name is a live NUL-terminated path to this test's unused private file.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let fifo_link = root.join("fifo-link");
    std::os::unix::fs::symlink(&fifo, &fifo_link).unwrap();
    for refused_path in [&fifo, &fifo_link, &root, Path::new("/dev/null")] {
        assert_eq!(
            upgrade_line(refused_path, "tool", Some(9000), &mut diagnostics),
            "the upgrade record could not be read"
        );
    }
    std::fs::write(&record, vec![b'x'; UPGRADE_BYTES as usize + 1]).unwrap();
    assert_eq!(
        upgrade_line(&record, "tool", Some(9000), &mut diagnostics),
        "the upgrade record could not be read"
    );
    assert!(!diagnostics.is_empty());
    assert_eq!(
        upgrade_line(&root.join("absent"), "tool", Some(9000), &mut diagnostics),
        "no upgrade record on this machine"
    );
}

#[test]
fn malformed_upgrade_text_loses_only_the_correlation() {
    let root = scratch();
    let record = root.join("record");
    for bytes in [
        b"9000\t1970-01-01T02:30:00Z\ntool\tadded".as_slice(),
        b"9000\t1970-01-01T02:30:00Z\n\0",
        b"\xff",
    ] {
        std::fs::write(&record, bytes).unwrap();
        let mut diagnostics = Vec::new();
        assert_eq!(
            upgrade_line(&record, "tool", Some(9000), &mut diagnostics),
            "the upgrade record could not be read"
        );
        assert!(!diagnostics.is_empty());
    }
}
