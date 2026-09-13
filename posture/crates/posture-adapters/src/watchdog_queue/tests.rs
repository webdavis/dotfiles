use super::*;
use rusqlite::Connection;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
fn path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "posture-watchdog-queue-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    fs::canonicalize(dir).unwrap().join("queue.db")
}
fn pns(path: &std::path::Path) -> Connection {
    let db = Connection::open(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    db.execute_batch("PRAGMA user_version=8; CREATE TABLE ledger_legs(id INTEGER PRIMARY KEY, acknowledged INTEGER NOT NULL, deadlettered_at BLOB); INSERT INTO ledger_legs VALUES(1,0,NULL),(2,1,NULL),(3,0,x'0000000000000001');").unwrap();
    db.execute_batch("CREATE TABLE delivery_health(id INTEGER PRIMARY KEY CHECK(id = 1), previous_pending INTEGER,
        growth INTEGER NOT NULL DEFAULT 0 CHECK(growth BETWEEN 0 AND 2),
        generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
        acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged >= 0 AND acknowledged <= generation));
        INSERT INTO delivery_health(id) VALUES(1);").unwrap();
    db
}
#[test]
fn missing_legacy_is_empty_but_missing_pns_is_unreadable_without_creation() {
    let path = path();
    assert_eq!(
        QueueDatabase::legacy(path.clone()).counts(),
        QueueCounts {
            pending: Some(0),
            deadletters: Some(0),
            alarm_generation: None,
        }
    );
    assert_eq!(
        QueueDatabase::pns(path.clone()).counts(),
        QueueCounts {
            pending: None,
            deadletters: None,
            alarm_generation: None,
        }
    );
    assert!(!path.exists());
}
#[test]
fn legacy_lazy_tables_are_empty_and_existing_counts_are_independent() {
    let path = path();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE pending_alerts(id); INSERT INTO pending_alerts VALUES(1),(2);")
        .unwrap();
    assert_eq!(
        QueueDatabase::legacy(path.clone()).counts(),
        QueueCounts {
            pending: Some(2),
            deadletters: Some(0),
            alarm_generation: None,
        }
    );
    db.execute_batch(
        "CREATE TABLE dead_letter_alerts(id); INSERT INTO dead_letter_alerts VALUES(3);",
    )
    .unwrap();
    assert_eq!(
        QueueDatabase::legacy(path).counts(),
        QueueCounts {
            pending: Some(2),
            deadletters: Some(1),
            alarm_generation: None,
        }
    );
}
#[test]
fn pns_counts_unacknowledged_obligations_and_retained_deadletters_directly() {
    let path = path();
    let db = pns(&path);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        QueueDatabase::pns(path.clone()).counts(),
        QueueCounts {
            pending: Some(1),
            deadletters: Some(1),
            alarm_generation: None,
        }
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        8
    );
}
#[test]
fn readers_include_committed_uncheckpointed_wal_rows() {
    let path = path();
    let db = pns(&path);
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; INSERT INTO ledger_legs VALUES(4,0,NULL);").unwrap();
    assert!(
        path.with_file_name("queue.db-wal")
            .metadata()
            .unwrap()
            .len()
            > 0
    );
    assert_eq!(QueueDatabase::pns(path).counts().pending, Some(2));
}
#[test]
fn corrupt_incompatible_and_locked_ledgers_refuse_without_repair() {
    for contents in [b"broken sqlite".as_slice(), b""] {
        let path = path();
        fs::write(&path, contents).unwrap();
        assert_eq!(QueueDatabase::pns(path.clone()).counts().pending, None);
        assert_eq!(fs::read(path).unwrap(), contents);
    }
    let file = path();
    let db = pns(&file);
    db.execute_batch("PRAGMA user_version=999").unwrap();
    assert_eq!(QueueDatabase::pns(file.clone()).counts().pending, None);
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        999
    );
    db.execute_batch("PRAGMA user_version=8; BEGIN EXCLUSIVE")
        .unwrap();
    let start = Instant::now();
    let mut reader = QueueDatabase::pns(file);
    reader.timeout = Duration::from_millis(5);
    assert_eq!(reader.counts().pending, None);
    assert!(start.elapsed() < Duration::from_secs(1));
}
#[test]
fn legacy_corruption_is_never_disguised_as_an_empty_lazy_table() {
    let path = path();
    fs::write(&path, b"broken sqlite").unwrap();
    assert_eq!(
        QueueDatabase::legacy(path).counts(),
        QueueCounts {
            pending: None,
            deadletters: None,
            alarm_generation: None,
        }
    );
}

#[test]
fn a_hostile_query_cannot_exceed_the_reader_budget() {
    let path = path();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE VIEW pending_alerts AS WITH RECURSIVE sequence(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM sequence) SELECT n FROM sequence;").unwrap();
    let mut reader = QueueDatabase::legacy(path);
    reader.timeout = Duration::from_millis(5);
    let start = Instant::now();
    assert_eq!(reader.counts().pending, None);
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn readable_pns_ledger_does_not_require_write_permission() {
    let path = path();
    let db = pns(&path);
    drop(db);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    assert_eq!(QueueDatabase::pns(path).counts().pending, Some(1));
}

#[test]
fn the_health_connection_cannot_write_even_to_a_writable_fixture() {
    let path = path();
    let db = pns(&path);
    drop(db);
    let mut reader = QueueDatabase::pns(path);
    let connection = reader.open().unwrap();
    let error = connection
        .execute("INSERT INTO ledger_legs VALUES(99,0,NULL)", [])
        .unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
    assert_eq!(reader.counts().pending, Some(1));
}

#[test]
fn missing_delivery_health_metadata_makes_the_ledger_unreadable() {
    let path = path();
    let db = pns(&path);
    db.execute_batch("DROP TABLE delivery_health").unwrap();
    assert_eq!(QueueDatabase::pns(path).counts().pending, None);
}

#[test]
fn missing_required_delivery_health_row_makes_the_ledger_unreadable() {
    let path = path();
    let db = pns(&path);
    db.execute_batch("DELETE FROM delivery_health").unwrap();
    assert_eq!(QueueDatabase::pns(path).counts().pending, None);
}

#[test]
fn invalid_delivery_health_values_are_never_an_all_clear() {
    for values in [
        "-1,0,0",
        "3,0,0",
        "0,-1,0",
        "0,0,-1",
        "0,0,1",
        "'bad',0,0",
        "NULL,0,0",
    ] {
        let path = path();
        let db = pns(&path);
        db.execute_batch(&format!(
            "DROP TABLE delivery_health;
            CREATE TABLE delivery_health(id,growth,generation,acknowledged);
            INSERT INTO delivery_health VALUES(1,{values});"
        ))
        .unwrap();
        assert_eq!(QueueDatabase::pns(path).counts().pending, None, "{values}");
    }
}

#[test]
fn outstanding_delivery_alarm_is_reported_without_acknowledging_or_changing_counts() {
    use posture_domain::{QueueKind, QueueMemory, judge_queue};
    let path = path();
    let db = pns(&path);
    db.execute_batch(
        "DELETE FROM ledger_legs; UPDATE delivery_health SET generation=8, acknowledged=7;",
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    let counts = QueueDatabase::pns(path.clone()).counts();
    assert_eq!((counts.pending, counts.deadletters), (Some(0), Some(0)));
    assert!(
        !judge_queue(QueueKind::Pns, counts, QueueMemory::default())
            .1
            .is_empty()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    db.execute_batch("UPDATE delivery_health SET acknowledged=8")
        .unwrap();
    let counts = QueueDatabase::pns(path).counts();
    assert!(
        judge_queue(QueueKind::Pns, counts, QueueMemory::default())
            .1
            .is_empty()
    );
}
