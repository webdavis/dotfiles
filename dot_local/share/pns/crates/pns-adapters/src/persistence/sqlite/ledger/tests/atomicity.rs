use super::*;
#[test]
fn a_failed_leg_insert_rolls_back_the_event_routes_and_initial_attempts() {
    let store = SqliteStore::new(state());
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON ledger_legs WHEN NEW.position = 1 BEGIN SELECT RAISE(ABORT, 'injected route failure'); END;").unwrap();
    let input = submission();
    assert!(matches!(
        store.prepare(&input, lease(10, 20)),
        Err(LedgerFailure::Unavailable(_))
    ));
    assert_eq!(
        store.inspect(&input.identity).unwrap(),
        None,
        "write-ahead plan is atomic"
    );
    for table in ["ledger_events", "ledger_legs", "ledger_attempts"] {
        assert_eq!(
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0,
            "{table} must roll back"
        );
    }
}
#[test]
fn a_failed_outcome_write_preserves_the_unfinished_claim_for_a_later_record() {
    let store = SqliteStore::new(state());
    let input = submission();
    let legs = created(&store, &input);
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER reject_ack BEFORE UPDATE ON ledger_attempts WHEN NEW.outcome = 1 BEGIN SELECT RAISE(ABORT, 'injected outcome failure'); END;").unwrap();
    assert!(matches!(
        store.record(&legs[0].claim, &acknowledged(), 11),
        Err(LedgerFailure::Unavailable(_))
    ));
    assert!(matches!(
        store.inspect(&input.identity).unwrap().unwrap().attempts[0].completion,
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Unknown,
            ..
        }
    ));
    connection.execute_batch("DROP TRIGGER reject_ack").unwrap();
    store
        .record(&legs[0].claim, &acknowledged(), 12)
        .expect("failed record must leave its claim owned");
}
#[test]
fn a_version_one_database_upgrades_without_changing_its_existing_records() {
    let path = state();
    std::fs::create_dir_all(&path).unwrap();
    let connection = rusqlite::Connection::open(path.join("pns.db")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path.join("pns.db"), std::fs::Permissions::from_mode(0o600)).unwrap();
    connection.execute_batch("CREATE TABLE decisions(seq INTEGER PRIMARY KEY,line TEXT NOT NULL); INSERT INTO decisions(line) VALUES ('old record'); PRAGMA user_version = 1;").unwrap();
    let store = SqliteStore::new(path);
    created(&store, &submission());
    assert_eq!(
        connection
            .query_row("SELECT line FROM decisions", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "old record"
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        3
    );
}
#[test]
fn a_failed_version_two_schema_upgrade_rolls_back_its_tables_and_version() {
    let path = state();
    std::fs::create_dir_all(&path).unwrap();
    let connection = rusqlite::Connection::open(path.join("pns.db")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path.join("pns.db"), std::fs::Permissions::from_mode(0o600)).unwrap();
    connection
        .execute_batch("CREATE TABLE ledger_legs(id INTEGER PRIMARY KEY); PRAGMA user_version = 1;")
        .unwrap();
    assert!(
        SqliteStore::new(path).connect().is_err(),
        "conflicting schema must refuse the migration"
    );
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'ledger_events'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0,
        "partial migration must roll back"
    );
}
