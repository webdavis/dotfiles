use super::{SqliteStore, state};
use pns_domain::missed::OpenWait;
use pns_domain::recap::activity::Event;

/// One activity row for a session, in a state, at a second.
fn row(store: &SqliteStore, session: &str, state: &str, at: u64, detail: &str) {
    store
        .record_activity_event(&Event {
            at,
            agent: "codex".into(),
            state: state.into(),
            project: "dotfiles".into(),
            session: session.into(),
            detail: detail.into(),
            ..Event::default()
        })
        .unwrap();
}

#[test]
fn only_a_session_whose_wait_is_still_open_is_listed_once_with_its_count() {
    let store = SqliteStore::new(state());
    // STILL WAITING: three asks while away, the newest one last.
    for (at, asks) in [
        (110, "Bash: ls"),
        (120, "Bash: rm"),
        (130, "Bash: git push"),
    ] {
        row(&store, "open", "blocked", at, asks);
        store.begin_wait("open", at, true).unwrap();
    }
    // ANSWERED: it asked, and a later event ended the wait.
    row(&store, "answered", "blocked", 115, "Bash: make");
    store.begin_wait("answered", 115, true).unwrap();
    row(&store, "answered", "done", 125, "");
    store.end_wait("answered").unwrap();

    assert_eq!(
        store.open_waits(100, 200).unwrap(),
        [OpenWait {
            agent: "codex".into(),
            state: "blocked".into(),
            project: "dotfiles".into(),
            asks: "Bash: git push".into(),
            count: 3,
        }]
    );
}

#[test]
fn a_wait_raised_before_the_absence_is_still_listed_and_counts_once() {
    let store = SqliteStore::new(state());
    row(&store, "early", "asked", 50, "which branch?");
    store.begin_wait("early", 50, true).unwrap();

    let open = store.open_waits(100, 200).unwrap();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].asks, "which branch?");
    assert_eq!(open[0].count, 1);
}

#[test]
fn open_waits_are_listed_oldest_first() {
    let store = SqliteStore::new(state());
    row(&store, "newer", "blocked", 150, "second");
    store.begin_wait("newer", 150, true).unwrap();
    row(&store, "older", "blocked", 140, "first");
    store.begin_wait("older", 140, true).unwrap();

    let asks: Vec<String> = store
        .open_waits(100, 200)
        .unwrap()
        .into_iter()
        .map(|wait| wait.asks)
        .collect();
    assert_eq!(asks, ["first", "second"]);
}

#[test]
fn a_wait_is_listed_whether_or_not_the_escalation_is_on() {
    let store = SqliteStore::new(state());
    row(&store, "paged", "blocked", 110, "Bash: git push");
    store.begin_wait("paged", 110, true).unwrap();
    row(&store, "quiet", "asked", 120, "which branch?");
    store.begin_wait("quiet", 120, false).unwrap();

    let asks: Vec<String> = store
        .open_waits(100, 200)
        .unwrap()
        .into_iter()
        .map(|wait| wait.asks)
        .collect();
    assert_eq!(asks, ["Bash: git push", "which branch?"]);
}
