use super::*;

/// The repair half of migration step 8, against a database carrying the CHECK
/// constraints AS THEY SHIPPED. The migration steps that wrote those spellings
/// now write the wide ones directly, so the legacy shape has to be spelled out
/// here or this would test the migration against its own fix.
#[test]
fn a_database_holding_the_shipped_narrow_constraints_is_widened_without_losing_its_rows() {
    use std::os::unix::fs::PermissionsExt;
    let path = state();
    std::fs::create_dir_all(&path).unwrap();
    let database = path.join("pns.db");
    let mut connection = rusqlite::Connection::open(&database).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();
    let input = remote_input();
    {
        let transaction = connection.transaction().unwrap();
        schema::create(&transaction).unwrap();
        schema::retain_request(&transaction).unwrap();
        transaction.execute_batch(
            "ALTER TABLE ledger_legs ADD COLUMN deadlettered_at BLOB CHECK(deadlettered_at IS NULL OR length(deadlettered_at) = 8);
             ALTER TABLE ledger_legs ADD COLUMN deadletter_reason TEXT CHECK(deadletter_reason IN ('attempts','age'));
             CREATE TABLE delivery_health(id INTEGER PRIMARY KEY CHECK(id = 1), previous_pending INTEGER,
               growth INTEGER NOT NULL DEFAULT 0 CHECK(growth BETWEEN 0 AND 2),
               generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
               acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged >= 0 AND acknowledged <= generation));
             INSERT INTO delivery_health(id) VALUES (1);
             ALTER TABLE ledger_legs ADD COLUMN http_status INTEGER CHECK(http_status IN (401,403,404,413));
             ALTER TABLE ledger_attempts ADD COLUMN http_status INTEGER CHECK(http_status IN (401,403,404,413));",
        ).unwrap();
        super::super::prepare::submission(&transaction, &input, lease(10, 20)).unwrap();
        transaction
            .execute(
                "UPDATE ledger_legs SET deadlettered_at = ?1, deadletter_reason = 'age',
                 http_status = 413, owner = NULL, token = NULL, lease_until = NULL",
                [11u64.to_be_bytes()],
            )
            .unwrap();
        transaction.pragma_update(None, "user_version", 7).unwrap();
        transaction.commit().unwrap();
    }

    let store = SqliteStore::new(path);
    let record = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(record.submission, input, "the submission survives");
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(store.delivery_health().unwrap().deadlettered_legs, 1);
    let retained: (String, Option<u16>) = connection
        .query_row(
            "SELECT deadletter_reason,http_status FROM ledger_legs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        retained,
        ("age".into(), Some(413)),
        "the old values survive"
    );

    // The point of the widening: values the shipped constraints refused.
    connection
        .execute(
            "UPDATE ledger_legs SET deadletter_reason = 'permanent', http_status = 502",
            [],
        )
        .expect("the widened leg constraints accept the new spellings");
    connection
        .execute("UPDATE ledger_attempts SET http_status = 502", [])
        .expect("the widened attempt constraint accepts any real status");
    // Still a constraint, not an open field.
    assert!(
        connection
            .execute("UPDATE ledger_legs SET http_status = 42", [])
            .is_err(),
        "a value that is not an HTTP status is still refused"
    );
}

#[test]
fn schema_six_http_migration_preserves_existing_deadletters_health_metadata_and_attempts() {
    use std::os::unix::fs::PermissionsExt;
    let path = state();
    std::fs::create_dir_all(&path).unwrap();
    let database = path.join("pns.db");
    let mut connection = rusqlite::Connection::open(&database).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();
    let input = remote_input();
    {
        let transaction = connection.transaction().unwrap();
        schema::create(&transaction).unwrap();
        schema::retain_request(&transaction).unwrap();
        schema::retain_deadletters(&transaction).unwrap();
        super::super::prepare::submission(&transaction, &input, lease(10, 20)).unwrap();
        transaction.execute("UPDATE ledger_legs SET deadlettered_at = ?1, deadletter_reason = 'attempts', owner = NULL, token = NULL, lease_until = NULL", [11u64.to_be_bytes()]).unwrap();
        transaction.execute_batch("UPDATE delivery_health SET previous_pending = 2, growth = 2, generation = 8, acknowledged = 7;").unwrap();
        transaction.pragma_update(None, "user_version", 6).unwrap();
        transaction.commit().unwrap();
    }
    let store = SqliteStore::new(path);
    assert!(
        store.delivery_health().is_err(),
        "read-only health cannot migrate schema six"
    );
    let record = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(record.submission, input);
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(
        (record.attempts[0].generation, record.attempts[0].at),
        (1, 10)
    );
    let health = store.delivery_health().unwrap();
    assert_eq!(
        (
            health.pending_legs,
            health.deadlettered_legs,
            health.growth_streak,
            health.alarm_generation
        ),
        (0, 1, 2, Some(8))
    );
    let retained: (String, Option<u16>) = connection
        .query_row(
            "SELECT deadletter_reason,http_status FROM ledger_legs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(retained, ("attempts".into(), None));
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        crate::persistence::sqlite::migrations::VERSION
    );
}
