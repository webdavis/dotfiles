use super::{SqliteStore, state};
use pns_application::{DecisionRing, Journal};
use std::fs;

#[test]
fn the_first_consumer_read_imports_existing_records_without_changing_legacy_bytes() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let history = "9 prior decision\r\n10 no final newline";
    fs::write(path.join("decisions"), history).unwrap();
    fs::write(path.join("quiet-until"), "23\n").unwrap();
    let store = SqliteStore::for_records(path.clone());
    assert!(!path.join("pns.db").exists(), "construction is inert");
    assert_eq!(
        DecisionRing::read(&store).unwrap().as_deref(),
        Some(history)
    );
    assert_eq!(store.quiet_expiry().unwrap(), Some(23));
    assert_eq!(Journal::read(&store).unwrap(), None);
    assert_eq!(
        fs::read(path.join("decisions")).unwrap(),
        history.as_bytes()
    );
    assert_eq!(fs::read(path.join("quiet-until")).unwrap(), b"23\n");
}

#[test]
fn an_active_legacy_owner_refuses_consumer_reads_and_writes_without_an_empty_success() {
    let path = state();
    fs::create_dir(&path).unwrap();
    let hold = path.join(format!(
        "missed-notifications.held.{}.0",
        std::process::id()
    ));
    fs::write(&hold, "held batch\n").unwrap();
    fs::write(path.join("decisions"), "prior decision\n").unwrap();
    let store = SqliteStore::for_records(path.clone());
    assert!(
        DecisionRing::read(&store).is_err(),
        "refusal is not empty history"
    );
    assert!(
        store.set_quiet_expiry(Some(99)).is_err(),
        "do not mutate before import"
    );
    let connection = SqliteStore::new(path.clone()).connect().unwrap();
    let imported: u32 = connection
        .query_row("SELECT count(*) FROM legacy_imports", [], |r| r.get(0))
        .unwrap();
    let quiet: u32 = connection
        .query_row("SELECT count(*) FROM quiet", [], |r| r.get(0))
        .unwrap();
    assert_eq!((imported, quiet), (0, 0));
    assert_eq!(fs::read(hold).unwrap(), b"held batch\n");
}

#[test]
fn completed_consumer_import_ignores_later_legacy_edits_and_needs_no_writer_lock_to_read() {
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("decisions"), "retained decision\n").unwrap();
    fs::write(path.join("quiet-until"), "23\n").unwrap();
    let store = SqliteStore::for_records(path.clone());
    assert_eq!(store.quiet_expiry().unwrap(), Some(23));
    store.set_quiet_expiry(None).unwrap();
    fs::write(path.join("quiet-until"), "99\n").unwrap();
    fs::write(path.join("decisions.lock"), "later old writer").unwrap();
    let connection = SqliteStore::new(path.clone()).connect().unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let reopened = SqliteStore::for_records(path);
    assert_eq!(reopened.quiet_expiry().unwrap(), None);
    assert_eq!(
        DecisionRing::read(&reopened).unwrap().as_deref(),
        Some("retained decision\n")
    );
    connection.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn consumer_import_retains_unknown_families_and_reports_them_without_blocking_other_records() {
    let path = state();
    fs::create_dir(&path).unwrap();
    fs::create_dir(path.join("decisions")).unwrap();
    fs::write(path.join("quiet-until"), "23\n").unwrap();
    let mut store = SqliteStore::for_records(path.clone());
    store.log = path.join("import.log");
    assert!(DecisionRing::read(&store).is_err());
    assert_eq!(store.quiet_expiry().unwrap(), Some(23));
    assert_eq!(
        store
            .import_failures()
            .unwrap()
            .iter()
            .map(|r| r.record.as_str())
            .collect::<Vec<_>>(),
        ["decisions"]
    );
    assert!(
        fs::read_to_string(&store.log)
            .unwrap()
            .contains("decisions")
    );
    assert!(path.join("decisions").is_dir());
}
