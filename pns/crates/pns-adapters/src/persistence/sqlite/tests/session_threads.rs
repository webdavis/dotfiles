use super::{SqliteStore, state};
use crate::destinations::discord::SessionThreads;

#[test]
fn a_stored_thread_round_trips_and_forgetting_clears_it() {
    let store = SqliteStore::new(state());
    assert_eq!(store.thread("s1", "c1"), None);
    store.remember("s1", "c1", "t-1");
    assert_eq!(store.thread("s1", "c1"), Some("t-1".to_string()));
    store.forget("s1", "c1");
    assert_eq!(store.thread("s1", "c1"), None);
}

#[test]
fn a_conflicting_remember_keeps_the_first_thread_instead_of_the_second() {
    // THE RACE THIS PINS: two concurrent first-events of one pair each open
    // their own thread and both call remember. First writer wins so every
    // later lookup converges on one thread rather than flapping between the
    // two the race created.
    let store = SqliteStore::new(state());
    store.remember("s1", "c1", "t-first");
    store.remember("s1", "c1", "t-second");
    assert_eq!(store.thread("s1", "c1"), Some("t-first".to_string()));
}
