use super::*;
use crate::StoreError;

#[test]
fn an_interrupted_return_owner_refuses_claim_and_completion_without_changing_its_records() {
    let store = SqliteStore::new(state());
    let connection = store.connect().unwrap();
    store.record_journal(&event("owned"), Some(1)).unwrap();
    store.claim_return(Some(2), true).unwrap().unwrap();
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = store.claim.lock().unwrap();
        panic!("owned interruption while holding return ownership");
    }));
    assert!(interrupted.is_err());
    let claim = store.claim_return(Some(3), true);
    let completion = store.complete_return();
    assert!(
        matches!(claim, Err(StoreError::InvalidState(_))),
        "interrupted ownership must refuse a new claim"
    );
    assert!(
        matches!(completion, Err(StoreError::InvalidState(_))),
        "interrupted ownership must refuse completion"
    );
    assert_eq!(store.last_present().unwrap(), Some(2));
    let records: (u32, u32) = connection
        .query_row(
            "SELECT (SELECT count(*) FROM journal), (SELECT count(*) FROM return_claims)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(records, (1, 1));
}

#[test]
fn concurrent_callers_share_one_return_owner_until_that_batch_completes() {
    let store = SqliteStore::new(state());
    store.record_journal(&event("one batch"), Some(1)).unwrap();
    let ready = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            ready.wait();
            store.claim_return(Some(2), true)
        });
        let second = scope.spawn(|| {
            ready.wait();
            store.claim_return(Some(3), true)
        });
        [
            first.join().unwrap().unwrap(),
            second.join().unwrap().unwrap(),
        ]
    });
    assert_eq!(
        results.iter().filter(|claim| claim.is_some()).count(),
        1,
        "one shared owner grants one concurrent return"
    );
    let claim = results.into_iter().flatten().next().unwrap();
    assert_eq!(claim.waiting.len(), 1);
    assert_eq!(claim.waiting[0].detail, "one batch");
    store.complete_return().unwrap();
    let counts: (u32, u32) = store
        .connect()
        .unwrap()
        .query_row(
            "SELECT (SELECT count(*) FROM journal), (SELECT count(*) FROM return_claims)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        counts,
        (0, 0),
        "the concurrent refusal must not lose the first owner's completion"
    );
}

#[test]
fn a_refused_completion_keeps_the_shared_owner_until_its_transaction_can_commit() {
    let mut store = SqliteStore::new(state());
    store.busy_timeout = Duration::from_millis(5);
    let mut observer = store.connect().unwrap();
    store.record_journal(&event("retained"), Some(1)).unwrap();
    store.claim_return(Some(2), true).unwrap().unwrap();
    let writer = observer
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    assert!(store.complete_return().is_err());
    writer.rollback().unwrap();
    assert!(
        store.claim_return(Some(3), true).unwrap().is_none(),
        "a failed completion must retain its in-memory claim"
    );
    store.complete_return().unwrap();
    assert!(
        store
            .claim_return(Some(4), true)
            .unwrap()
            .unwrap()
            .waiting
            .is_empty()
    );
}
