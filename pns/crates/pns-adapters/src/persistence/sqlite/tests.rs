use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
/// A path no other test and no other RUN can build.
///
/// MEASURED: a process id is recycled and these directories are never
/// removed, so a counter alone rebuilt paths an earlier run of a recycled
/// id had already populated. Replaying one run's 160 leftover databases
/// into the next run's paths failed 84 rows, 22 of them AlreadyExists on
/// the directory itself and the rest reading the earlier run's records.
/// The epoch nanosecond is the same component `state_fixtures::scratch`
/// uses, which cannot be reused here because it creates the directory and
/// two rows below need to create it themselves.
fn state() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("the canonical temp directory")
        .join(format!(
            "pns-sql-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos()),
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
    // Read the constant, not a literal: what this pins is that a fresh database
    // lands on the CURRENT version, and the literal made every migration edit
    // three unrelated tests.
    assert_eq!(version, migrations::VERSION);
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
    connection
        .pragma_update(None, "user_version", migrations::VERSION + 1)
        .unwrap();
    assert!(store.connect().is_err(), "future schema must be refused");
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        migrations::VERSION + 1
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
    connection
        .pragma_update(None, "user_version", migrations::VERSION + 1)
        .unwrap();
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

mod sessions;

mod session_threads;

#[test]
fn an_open_that_loses_the_wal_conversion_still_records() {
    // MEASURED: `PRAGMA journal_mode=WAL` answers SQLITE_BUSY instantly when
    // another connection holds the write lock on a rollback-journal database,
    // whatever the connection's `busy_timeout` says, and treating that refusal
    // as fatal aborted the whole open. Every record that open was about to
    // write went with it, fail-quiet, which is a lost notification.
    //
    // STAGED RATHER THAN RACED: an unstaged race needed thousands of fresh
    // opens to lose one, and this pins the same statement in milliseconds.
    let state = state();
    let back_to_rollback = SqliteStore::new(state.clone())
        .connect()
        .expect("the schema");
    back_to_rollback
        .pragma_update(None, "journal_mode", "DELETE")
        .expect("a rollback-journal database, the shape a conversion can lose");
    drop(back_to_rollback);
    let holder = rusqlite::Connection::open(state.join("pns.db")).expect("another writer");
    holder.execute_batch("BEGIN IMMEDIATE").expect("its lock");

    let connection = SqliteStore::new(state)
        .connect()
        .expect("a conversion this open lost is not the open's problem");
    assert_eq!(
        connection
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .expect("the mode it settled for"),
        "delete",
        "the refusal has to be a real one, not a conversion that quietly won"
    );

    drop(holder);
    connection
        .execute(
            "INSERT INTO decisions(line) VALUES ('after the refusal')",
            [],
        )
        .expect("the open that lost the conversion still writes records");
}
