use super::*;
use std::time::{Duration, Instant};
#[test]
fn busy_ledger_writes_refuse_within_the_budget_without_recording_sensitive_content() {
    let path = state();
    let mut store = SqliteStore::new(path.clone());
    store.busy_timeout = Duration::from_millis(5);
    store.log = path.join("private.log");
    let mut connection = store.connect().unwrap();
    let _hold = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let started = Instant::now();
    let result = store.prepare(&submission(), lease(10, 20));
    assert!(matches!(result, Err(LedgerFailure::Unavailable(_))));
    assert!(started.elapsed() < Duration::from_millis(100));
    let log = std::fs::read_to_string(&store.log).unwrap();
    assert!(log.contains("database busy"));
    assert!(!log.contains("original-id"));
    assert!(!log.contains("second"));
}
