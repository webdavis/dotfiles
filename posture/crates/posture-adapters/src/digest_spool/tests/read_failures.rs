use super::*;
use std::io::ErrorKind;

#[test]
fn invalid_utf8_preserves_the_claim_and_its_valid_neighbors_across_recovery() {
    let fixture = Fixture::new();
    let mut bytes = format!("{}\n", line("alpha", "one")).into_bytes();
    bytes.extend_from_slice(b"\xff\n");
    bytes.extend_from_slice(format!("{}\n", line("beta", "two")).as_bytes());
    fs::write(&fixture.store, &bytes).unwrap();
    assert_eq!(
        fs::read_to_string(&fixture.store).unwrap_err().kind(),
        ErrorKind::InvalidData
    );
    let first = fixture.spool(100);
    assert!(first.claim().is_none());
    assert_eq!(fs::read(first.claim_path()).ok(), Some(bytes.clone()));

    let later = format!("{}\n", line("gamma", "three"));
    fixture.write_spool(&later);
    let expected = [later.as_bytes(), bytes.as_slice()].concat();
    let next = fixture.spool(200);
    next.sweep_orphans();
    assert_eq!(fs::read(&fixture.store).unwrap(), expected);
    assert!(next.claim().is_none());
    assert_eq!(fs::read(next.claim_path()).ok(), Some(expected));
    assert!(fixture.kept().is_none());
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
    assert!(first.claim().is_none());
    let claimed = first.claim_path();
    assert!(
        claimed.exists(),
        "a denied read must retain its claimed batch"
    );
    fs::set_permissions(&claimed, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(fs::read(&claimed).unwrap(), before.as_bytes());

    fixture.write_spool(&format!("{}\n", line("beta", "two")));
    let next = fixture.spool(200);
    next.sweep_orphans();
    let recovered = next.claim().unwrap();
    assert_eq!(recovered.item_count, 2);
    assert_eq!(recovered.rows[0].identity.as_deref(), Some("two"));
    assert_eq!(recovered.rows[1].identity.as_deref(), Some("one"));
    assert!(!claimed.exists());
}

#[test]
fn a_denied_restore_read_preserves_both_batches_until_retry() {
    let fixture = Fixture::new();
    let before = format!("{}\n", line("alpha", "one"));
    fixture.write_spool(&before);
    let spool = fixture.spool(100);
    let batch = spool.claim().unwrap();
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
    assert_eq!(spool.claim().unwrap().item_count, 2);
}
