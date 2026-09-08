use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn state() -> PathBuf {
    std::env::temp_dir().join(format!(
        "pns-sql-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
#[test]
fn a_new_store_commits_its_schema_in_a_private_wal_database() {
    let state = state();
    let store = SqliteStore::new(state.clone());
    let connection = store.connect().expect("create the repository");
    let journal: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(journal, "wal");
    assert_eq!(version, 3);
    for name in ["pns.db", "pns.db-wal", "pns.db-shm"] {
        assert_eq!(
            std::fs::metadata(state.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "{name}"
        );
    }
}

#[test]
fn a_reopened_store_keeps_committed_records() {
    let state = state();
    let store = SqliteStore::new(state.clone());
    let connection = store.connect().unwrap();
    connection
        .execute("INSERT INTO decisions(line) VALUES ('old outcome')", [])
        .unwrap();
    drop(connection);
    let reopened = SqliteStore::new(state).connect().unwrap();
    let line: String = reopened
        .query_row("SELECT line FROM decisions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(line, "old outcome");
}

#[test]
fn a_newer_schema_is_refused_without_rewriting_its_version_or_data() {
    let state = state();
    let store = SqliteStore::new(state);
    let connection = store.connect().unwrap();
    connection
        .execute("INSERT INTO decisions(line) VALUES ('future outcome')", [])
        .unwrap();
    connection.pragma_update(None, "user_version", 4).unwrap();
    assert!(store.connect().is_err(), "future schema must be refused");
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        connection
            .query_row("SELECT line FROM decisions", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "future outcome"
    );
}

#[test]
fn a_symlink_database_is_refused_without_touching_its_target() {
    let state = state();
    std::fs::create_dir(&state).unwrap();
    let target_directory = state.join("other-state");
    drop(
        SqliteStore::new(target_directory.clone())
            .connect()
            .unwrap(),
    );
    let target = target_directory.join("pns.db");
    let before = std::fs::read(&target).unwrap();
    std::os::unix::fs::symlink(&target, state.join("pns.db")).unwrap();
    assert!(SqliteStore::new(state).connect().is_err());
    assert_eq!(std::fs::read(target).unwrap(), before);
}

#[test]
fn a_publicly_readable_database_is_refused_without_changing_its_permissions() {
    let state = state();
    let store = SqliteStore::new(state.clone());
    drop(store.connect().unwrap());
    let database = state.join("pns.db");
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.connect().is_err(), "do not expose stored event text");
    assert_eq!(
        std::fs::metadata(database).unwrap().permissions().mode() & 0o777,
        0o644
    );
}

#[test]
fn an_irregular_database_is_refused_without_opening_it() {
    let state = state();
    std::fs::create_dir_all(state.join("pns.db")).unwrap();
    assert!(SqliteStore::new(state.clone()).connect().is_err());
    assert!(state.join("pns.db").is_dir());
}

#[test]
fn a_future_schema_is_rejected_before_changing_its_journal_mode() {
    let state = state();
    let store = SqliteStore::new(state.clone());
    drop(store.connect().unwrap());
    let connection = rusqlite::Connection::open(state.join("pns.db")).unwrap();
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    connection.pragma_update(None, "user_version", 4).unwrap();
    drop(connection);
    assert!(store.connect().is_err());
    let connection = rusqlite::Connection::open(state.join("pns.db")).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .unwrap(),
        "delete"
    );
}

mod contracts;

mod failures;

mod processes;

mod returns;

mod lamps;
mod settings;

mod history;

mod import;

mod import_claims;

mod ports;

mod consumer_start;

mod decision_outcomes;

mod write_reports;
