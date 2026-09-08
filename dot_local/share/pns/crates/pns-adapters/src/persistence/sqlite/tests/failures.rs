use super::{SqliteStore, state};
use pns_application::Journal;
use pns_domain::EventArgs;
use rusqlite::TransactionBehavior;
use std::time::{Duration, Instant};

#[test]
fn a_failed_prune_rolls_back_the_new_record_and_preserves_the_whole_prior_ring() {
    let store = SqliteStore::new(state());
    let connection = store.connect().unwrap();
    for n in 0..25 {
        store
            .record_journal(
                &EventArgs {
                    detail: n.to_string(),
                    ..EventArgs::default()
                },
                Some(n),
            )
            .unwrap();
    }
    let before = Journal::read(&store).unwrap();
    connection.execute_batch("CREATE TRIGGER refuse_prune BEFORE DELETE ON journal BEGIN SELECT RAISE(ABORT, 'refuse prune'); END;").unwrap();
    assert!(
        store
            .record_journal(
                &EventArgs {
                    detail: "new".into(),
                    ..EventArgs::default()
                },
                Some(26)
            )
            .is_err()
    );
    assert_eq!(
        Journal::read(&store).unwrap(),
        before,
        "failed transaction must preserve every prior row and omit the new row"
    );
}

#[test]
fn a_busy_delivery_record_returns_and_logs_the_miss_without_disclosing_event_text() {
    let state = state();
    let mut store = SqliteStore::new(state.clone());
    store.busy_timeout = Duration::from_millis(5);
    store.log = state.join("daemon.log");
    let mut blocker = store.connect().unwrap();
    let transaction = blocker
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let event = EventArgs {
        detail: "private event body".into(),
        ..EventArgs::default()
    };
    let started = Instant::now();
    assert!(
        store.record_journal(&event, Some(1)).is_err(),
        "a refused write must be returned as failure"
    );
    Journal::journal(&store, &event, Some(1));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "the hot path must not wait out SQLite's default five seconds"
    );
    let diagnostic = std::fs::read_to_string(&store.log)
        .expect("the missed record must reach the existing daemon log");
    assert!(
        diagnostic.contains("journal") && diagnostic.contains("busy"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("private event body"));
    transaction.rollback().unwrap();
    assert_eq!(Journal::read(&store).unwrap(), None);
}
