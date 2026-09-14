use super::*;
use std::io::ErrorKind;

#[test]
fn a_torn_line_is_dropped_so_the_lines_around_it_still_reach_the_digest() {
    // ONE TORN LINE USED TO WEDGE THE DIGEST FOREVER. The alerter appends from
    // another process, so a partial write can leave bytes that are not UTF-8,
    // and failing the whole read on them left the batch to be swept back,
    // re-claimed and failed again on every later run while the spool grew
    // behind it. The line is dropped instead, and only what arrived is counted.
    let fixture = Fixture::new();
    let mut bytes = format!("{}\n", line("alpha", "one")).into_bytes();
    bytes.extend_from_slice(b"\xff\n");
    bytes.extend_from_slice(format!("{}\n", line("beta", "two")).as_bytes());
    fs::write(&fixture.store, &bytes).unwrap();
    assert_eq!(
        fs::read_to_string(&fixture.store).unwrap_err().kind(),
        ErrorKind::InvalidData
    );

    let batch = claimed(&fixture.spool(100));
    assert_eq!(batch.rows.len(), 2);
    assert_eq!(batch.rows[0].detector.as_deref(), Some("alpha"));
    assert_eq!(batch.rows[1].detector.as_deref(), Some("beta"));
    assert_eq!(batch.item_count, 3);
}

#[test]
fn a_denied_claim_read_recovers_after_read_access_returns() {
    let fixture = Fixture::new();
    let before = format!("{}\n", line("alpha", "one"));
    fixture.write_spool(&before);
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o000)).unwrap();
    assert_eq!(
        fs::read_to_string(&fixture.store).unwrap_err().kind(),
        ErrorKind::PermissionDenied
    );
    let first = fixture.spool(100);
    // AN I/O FAILURE IS NOT AN EMPTY DAY. The bytes are still there and only
    // the read of them failed, so the run reports it rather than answering the
    // way a quiet day answers.
    let failure = first.claim().unwrap_err();
    let batch_path = first.claim_path();
    assert!(
        failure.0.contains(&batch_path.display().to_string()),
        "{failure}"
    );
    assert!(
        batch_path.exists(),
        "a denied read must retain its claimed batch"
    );
    fs::set_permissions(&batch_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(fs::read(&batch_path).unwrap(), before.as_bytes());

    fixture.write_spool(&format!("{}\n", line("beta", "two")));
    let next = fixture.spool(200);
    next.sweep_orphans();
    let recovered = claimed(&next);
    assert_eq!(recovered.item_count, 2);
    assert_eq!(recovered.rows[0].identity.as_deref(), Some("two"));
    assert_eq!(recovered.rows[1].identity.as_deref(), Some("one"));
    assert!(!batch_path.exists());
}

#[test]
fn a_denied_restore_read_preserves_both_batches_until_retry() {
    let fixture = Fixture::new();
    let before = format!("{}\n", line("alpha", "one"));
    fixture.write_spool(&before);
    let spool = fixture.spool(100);
    let batch = claimed(&spool);
    let later = format!("{}\n", line("beta", "two"));
    fixture.write_spool(&later);
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o200)).unwrap();
    assert_eq!(
        fs::read(&fixture.store).unwrap_err().kind(),
        ErrorKind::PermissionDenied
    );
    spool.restore(&batch);
    fs::set_permissions(&fixture.store, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(fs::read(&fixture.store).unwrap(), later.as_bytes());
    assert_eq!(fs::read(&batch.handle).ok(), Some(before.into_bytes()));

    spool.restore(&batch);
    assert_eq!(claimed(&spool).item_count, 2);
}
