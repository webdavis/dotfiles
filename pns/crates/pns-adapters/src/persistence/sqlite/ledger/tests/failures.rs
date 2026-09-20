use super::*;
use std::time::Duration;
#[test]
fn busy_ledger_writes_refuse_as_unavailable_without_recording_sensitive_content() {
    let path = state();
    let mut store = SqliteStore::new(path.clone());
    store.busy_timeout = Duration::from_millis(5);
    store.log = path.join("private.log");
    let mut connection = store.connect().unwrap();
    let _hold = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let result = store.prepare(&submission(), lease(10, 20));
    assert!(matches!(result, Err(LedgerFailure::Unavailable(_))));
    let log = std::fs::read_to_string(&store.log).unwrap();
    assert!(log.contains("database busy"));
    assert!(!log.contains("original-id"));
    assert!(!log.contains("second"));
}

use pns_application::SubmissionDelivery;

/// One hermes leg the gateway refuses, submitted and then retried, which is
/// what the classifier gives up on: a 404 is permanent, so the retry stops and
/// the leg is dead-lettered while still unacknowledged.
fn refused(status: u16) -> (SqliteStore, std::path::PathBuf) {
    let path = state();
    let store = SqliteStore::new(path.clone());
    let destinations = destinations(status);
    let delivery = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    delivery
        .submit(&remote_input(), None, lease(10, 40), &|| Some(10), &|_| {})
        .unwrap();
    delivery
        .retry(lease(100, 130), &|| Some(100), &|_| {})
        .unwrap()
        .unwrap();
    (store, path)
}

fn attempt_count(path: &std::path::Path) -> u32 {
    rusqlite::Connection::open(path.join("pns.db"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM ledger_attempts", [], |row| row.get(0))
        .unwrap()
}

/// A leg the retry policy gave up on is what the operator is stuck with, and
/// the drain is the only thing that takes it off the listing. What was tried
/// stays on record: the attempts are the audit trail of the delivery.
#[test]
fn draining_clears_a_dead_lettered_leg_and_keeps_its_attempts() {
    let (store, path) = refused(404);
    let listed = store.failing_legs(20).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].deadlettered);
    let attempts = attempt_count(&path);
    assert_eq!(attempts, 2);

    assert_eq!(store.drain_deadlettered_legs().unwrap(), 1);
    assert!(store.failing_legs(20).unwrap().is_empty());
    assert_eq!(store.failing_leg(listed[0].id).unwrap(), None);
    assert_eq!(attempt_count(&path), attempts);
}

/// A failing leg still inside its retry budget may yet arrive, so the drain
/// leaves it alone: clearing it would hide a delivery that is still chased.
#[test]
fn draining_leaves_a_leg_that_is_still_being_retried() {
    let (store, _path) = refused(503);
    assert_eq!(store.drain_deadlettered_legs().unwrap(), 0);
    let listed = store.failing_legs(20).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].deadlettered);
}

/// Nothing to drain is an answer rather than an error, which is what lets the
/// command be run blind.
#[test]
fn draining_a_ledger_with_nothing_dead_lettered_reports_none() {
    let store = SqliteStore::new(state());
    assert_eq!(store.drain_deadlettered_legs().unwrap(), 0);
}
