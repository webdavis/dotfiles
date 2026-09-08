use super::{SqliteStore, state};
use pns_application::{Journal, ReturnMoment};
use pns_domain::EventArgs;
use std::time::Duration;

fn event(detail: &str) -> EventArgs {
    EventArgs {
        detail: detail.into(),
        ..EventArgs::default()
    }
}
#[test]
fn the_return_edge_and_waiting_journal_are_claimed_together_and_completion_preserves_later_arrivals()
 {
    let state = state();
    let store = SqliteStore::new(state.clone());
    let _keep_wal_open = store.connect().unwrap();
    store.mark_present(10).unwrap();
    store.record_journal(&event("old"), Some(11)).unwrap();
    let first =
        ReturnMoment::claim(&store, Some(12), true).expect("the first return owns both records");
    assert_eq!(first.since, Some(10));
    assert_eq!(first.waiting[0].detail, "old");
    assert_eq!(store.last_present().unwrap(), Some(12));
    assert_eq!(
        Journal::read(&store).unwrap(),
        None,
        "claimed records are no longer pending"
    );
    assert!(
        ReturnMoment::claim(&store, Some(13), true).is_none(),
        "one owner must finish its previous batch"
    );
    store.record_journal(&event("later"), Some(13)).unwrap();
    ReturnMoment::complete(&store);
    assert!(Journal::read(&store).unwrap().unwrap().contains("later"));
    assert!(!Journal::read(&store).unwrap().unwrap().contains("old"));
    let next = SqliteStore::new(state)
        .claim_return(Some(14), true)
        .unwrap()
        .unwrap();
    assert_eq!(next.since, Some(12));
    assert_eq!(next.waiting.len(), 1);
    assert_eq!(next.waiting[0].detail, "later");
}
#[test]
fn claiming_only_the_return_edge_never_takes_the_journal_and_an_unknown_clock_never_moves_the_edge()
{
    let store = SqliteStore::new(state());
    let _keep_wal_open = store.connect().unwrap();
    store.mark_present(u64::MAX).unwrap();
    store.record_journal(&event("pending"), None).unwrap();
    for now in [None, Some(0), Some(u64::MAX - 1)] {
        let claim = store.claim_return(now, false).unwrap().unwrap();
        assert_eq!(claim.since, Some(u64::MAX));
        assert!(claim.waiting.is_empty());
        assert_eq!(store.last_present().unwrap(), Some(u64::MAX));
        assert!(Journal::read(&store).unwrap().unwrap().contains("pending"));
    }
}
#[test]
fn an_unfinished_return_is_preserved_when_its_store_drops_and_another_live_owner_cannot_take_it() {
    let state = state();
    let store = SqliteStore::new(state.clone());
    let connection = store.connect().unwrap();
    store.record_journal(&event("held"), Some(1)).unwrap();
    assert_eq!(
        store
            .claim_return(Some(2), true)
            .unwrap()
            .unwrap()
            .waiting
            .len(),
        1
    );
    drop(store);
    let other = SqliteStore::new(state);
    assert!(
        other
            .claim_return(Some(3), true)
            .unwrap()
            .unwrap()
            .waiting
            .is_empty()
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM journal", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        1,
        "dropping an unfinished owner must leave its batch recoverable"
    );
}
#[test]
fn a_busy_return_claim_fails_closed_without_advancing_the_edge_or_consuming_the_journal() {
    let mut store = SqliteStore::new(state());
    store.busy_timeout = Duration::from_millis(5);
    let mut connection = store.connect().unwrap();
    store.mark_present(1).unwrap();
    store.record_journal(&event("pending"), Some(2)).unwrap();
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    assert!(
        store.claim_return(Some(3), true).is_err(),
        "a write refusal must be distinct from an owned empty claim"
    );
    transaction.rollback().unwrap();
    assert_eq!(store.last_present().unwrap(), Some(1));
    assert!(Journal::read(&store).unwrap().unwrap().contains("pending"));
}

#[test]
fn pruning_later_arrivals_cannot_evict_the_batch_an_active_return_still_owns() {
    let store = SqliteStore::new(state());
    let connection = store.connect().unwrap();
    store
        .record_journal(&event("owned batch"), Some(1))
        .unwrap();
    store.claim_return(Some(2), true).unwrap().unwrap();
    for n in 0..26 {
        store
            .record_journal(&event(&format!("later {n}")), Some(n + 3))
            .unwrap();
    }
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM journal WHERE claim IS NOT NULL",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    let pending = crate::journal_codec::entries(&Journal::read(&store).unwrap().unwrap());
    assert_eq!(pending.len(), 25);
    assert_eq!(pending[0].detail, "later 1");
    store.complete_return().unwrap();
    assert_eq!(
        crate::journal_codec::entries(&Journal::read(&store).unwrap().unwrap()),
        pending
    );
}

mod shared_owner;
