use super::*;
use pns_domain::retry::RetryLimits;
use std::os::unix::fs::PermissionsExt;

fn one(store: &SqliteStore, id: &str) -> Vec<ClaimedLeg<DeliveryClaim>> {
    let mut input = submission();
    input.identity.request_id = id.into();
    input.legs.truncate(1);
    created(store, &input)
}
#[test]
fn delivery_health_retains_two_growth_samples_and_retries_unacknowledged_alarms_after_reopen() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    store.connect().unwrap();
    assert_eq!(store.sample_delivery_health().unwrap().pending_legs, 0);
    one(&store, "first");
    assert_eq!(store.sample_delivery_health().unwrap().growth_streak, 1);
    let reopened = SqliteStore::new(path);
    one(&reopened, "second");
    let growth = reopened.sample_delivery_health().unwrap();
    assert_eq!(growth.pending_legs, 2);
    assert_eq!(growth.growth_streak, 2);
    let pending = growth
        .alarm_generation
        .expect("two consecutive increases raise an alarm");
    let steady = reopened.sample_delivery_health().unwrap();
    assert_eq!(steady.growth_streak, 0);
    assert_eq!(steady.alarm_generation, Some(pending));
    reopened.acknowledge_delivery_alarm(pending).unwrap();
    assert_eq!(reopened.delivery_health().unwrap().alarm_generation, None);
    one(&reopened, "third");
    reopened.sample_delivery_health().unwrap();
    one(&reopened, "fourth");
    let newer = reopened
        .sample_delivery_health()
        .unwrap()
        .alarm_generation
        .unwrap();
    reopened.acknowledge_delivery_alarm(pending).unwrap();
    assert_eq!(
        reopened.delivery_health().unwrap().alarm_generation,
        Some(newer),
        "old acknowledgement cannot clear a newer finding"
    );
}
#[test]
fn deadlettering_preserves_metadata_routes_history_acknowledged_siblings_and_active_claims() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    input.producer_request = Some("canonical producer request".into());
    let legs = created(&store, &input);
    store.record(&legs[1].claim, &acknowledged(), 11).unwrap();
    let active = one(&store, "active");
    let before = store.inspect(&input.identity).unwrap().unwrap();
    let limits = RetryLimits {
        max_attempts: 0,
        max_age_secs: 0,
    };
    assert!(store.claim_retry(lease(19, 30), limits).unwrap().is_none());
    assert_eq!(
        store.delivery_health().unwrap().deadlettered_legs,
        0,
        "active lease must survive even exhausted limits"
    );
    store.record(&legs[0].claim, &retry(19), 19).unwrap();
    assert!(store.claim_retry(lease(19, 30), limits).unwrap().is_none());
    let after = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(before.submission, after.submission);
    assert_eq!(after.attempts.len(), 2);
    assert_eq!(after.attempts[1], before.attempts[1]);
    let health = store.delivery_health().unwrap();
    assert_eq!((health.pending_legs, health.deadlettered_legs), (1, 1));
    assert!(health.alarm_generation.is_some());
    let reason: String = store
        .connect()
        .unwrap()
        .query_row(
            "SELECT deadletter_reason FROM ledger_legs WHERE deadlettered_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        reason, "attempts",
        "attempt exhaustion takes precedence over age"
    );
    store.record(&active[0].claim, &acknowledged(), 19).unwrap();
    assert_eq!(store.delivery_health().unwrap().pending_legs, 0);
    assert!(matches!(
        store.prepare(&input, lease(40, 50)).unwrap(),
        PreparedSubmission::Existing(_)
    ));
}
#[test]
fn delivery_deadletter_update_and_alarm_are_atomic_and_do_not_starve_a_later_eligible_leg() {
    let store = SqliteStore::new(state());
    let old = one(&store, "old");
    store.record(&old[0].claim, &retry(20), 11).unwrap();
    let mut later = submission();
    later.identity.request_id = "later".into();
    later.legs.truncate(1);
    store.prepare(&later, lease(30, 40)).unwrap();
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER reject_alarm BEFORE UPDATE ON delivery_health BEGIN SELECT RAISE(ABORT,'private injected failure'); END;").unwrap();
    let limits = RetryLimits {
        max_attempts: 20,
        max_age_secs: 30,
    };
    assert!(store.claim_retry(lease(41, 51), limits).is_err());
    assert_eq!(
        store.delivery_health().unwrap().deadlettered_legs,
        0,
        "failed alarm write rolls back the terminal transition"
    );
    connection
        .execute_batch("DROP TRIGGER reject_alarm;")
        .unwrap();
    let claimed = store.claim_retry(lease(41, 51), limits).unwrap().unwrap();
    assert_eq!(claimed.identity, later.identity);
    assert_eq!(store.delivery_health().unwrap().deadlettered_legs, 1);
    assert!(matches!(
        store.record(&old[0].claim, &acknowledged(), 42),
        Err(LedgerFailure::LostClaim)
    ));
}
#[test]
fn delivery_health_read_only_refuses_missing_corrupt_and_old_schema_without_creating_or_migrating()
{
    let missing = state();
    let store = SqliteStore::new(missing.clone());
    assert!(store.delivery_health().is_err());
    assert!(!missing.exists());
    std::fs::create_dir_all(&missing).unwrap();
    let db = missing.join("pns.db");
    std::fs::write(&db, "corrupt").unwrap();
    std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(store.delivery_health().is_err());
    assert_eq!(std::fs::read(&db).unwrap(), b"corrupt");
    let old = state();
    std::fs::create_dir_all(&old).unwrap();
    let db = old.join("pns.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    connection.execute_batch("PRAGMA user_version=5;").unwrap();
    assert!(SqliteStore::new(old).delivery_health().is_err());
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        5
    );
}
#[test]
fn delivery_health_includes_committed_wal_rows_and_reports_recording_gaps_without_echoing_log_data()
{
    let mut store = SqliteStore::new(state());
    store.log = state().join("daemon.log");
    let connection = store.connect().unwrap();
    connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    one(&store, "wal");
    assert!(store.state.join("pns.db-wal").metadata().unwrap().len() > 0);
    assert_eq!(store.delivery_health().unwrap().pending_legs, 1);
    assert!(!store.delivery_health().unwrap().recording_gap);
    store.report_delivery_gap();
    assert!(store.delivery_health().unwrap().recording_gap);
    std::fs::write(&store.log, "unrelated private log value\n").unwrap();
    assert!(!store.delivery_health().unwrap().recording_gap);
}
#[test]
fn delivery_health_lock_contention_is_bounded_and_never_an_empty_snapshot() {
    let path = state();
    let mut store = SqliteStore::new(path.clone());
    store.busy_timeout = std::time::Duration::from_millis(10);
    let connection = store.connect().unwrap();
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    connection.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    let start = std::time::Instant::now();
    assert!(store.delivery_health().is_err());
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
    connection.execute_batch("ROLLBACK;").unwrap();
}

#[test]
fn schema_five_delivery_metadata_epochs_routes_and_attempts_survive_the_health_migration() {
    let path = state();
    std::fs::create_dir_all(&path).unwrap();
    let db = path.join("pns.db");
    let mut connection = rusqlite::Connection::open(&db).unwrap();
    std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut input = submission();
    input.producer_request = Some("canonical request bytes".into());
    {
        let transaction = connection.transaction().unwrap();
        schema::create(&transaction).unwrap();
        schema::retain_request(&transaction).unwrap();
        super::super::prepare::submission(&transaction, &input, lease(u64::MAX - 10, u64::MAX))
            .unwrap();
        transaction.pragma_update(None, "user_version", 5).unwrap();
        transaction.commit().unwrap();
    }
    let store = SqliteStore::new(path);
    let record = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(record.submission, input);
    assert!(
        record
            .attempts
            .iter()
            .all(|attempt| attempt.at == u64::MAX - 10)
    );
    assert_eq!(record.attempts.len(), 2);
    assert_eq!(store.delivery_health().unwrap().pending_legs, 2);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        6
    );
}
