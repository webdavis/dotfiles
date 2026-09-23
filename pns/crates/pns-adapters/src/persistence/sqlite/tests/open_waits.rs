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
    store.end_wait("answered", Some(125)).unwrap();

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
fn a_monday_wait_nobody_answered_is_not_on_wednesdays_card() {
    // THE CARD IS ABOUT THE ABSENCE: a wait left open on Monday by a session
    // that never sent another event is not something that happened while the
    // operator was away from Tuesday night to Wednesday.
    const DAY: u64 = 86_400;
    const MONDAY: u64 = 1_000 * DAY;
    const TUESDAY_NIGHT: u64 = MONDAY + DAY + DAY / 2;
    const WEDNESDAY: u64 = MONDAY + 2 * DAY;
    let store = SqliteStore::new(state());
    row(
        &store,
        "abandoned",
        "asking",
        MONDAY,
        "Should I open the PR?",
    );
    store.begin_wait("abandoned", MONDAY, true).unwrap();
    row(&store, "away", "blocked", WEDNESDAY - 60, "Bash: git push");
    store.begin_wait("away", WEDNESDAY - 60, true).unwrap();

    let asks: Vec<String> = store
        .open_waits(TUESDAY_NIGHT, WEDNESDAY)
        .unwrap()
        .into_iter()
        .map(|wait| wait.asks)
        .collect();
    assert_eq!(asks, ["Bash: git push"]);
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

#[test]
fn only_the_waits_nothing_answered_are_counted() {
    // EIGHT ASKS, SIX ANSWERED FROM THE PHONE: each answer is a later event
    // from the session, so only the two after the last one are outstanding.
    let store = SqliteStore::new(state());
    for at in (110..170).step_by(10) {
        row(&store, "s", "blocked", at, "Bash: make");
        store.begin_wait("s", at, true).unwrap();
        row(&store, "s", "resolved", at + 5, "");
        store.end_wait("s", Some(at + 5)).unwrap();
    }
    for (at, asks) in [(180, "Bash: ls"), (190, "Bash: git push")] {
        row(&store, "s", "blocked", at, asks);
        store.begin_wait("s", at, true).unwrap();
    }

    let open = store.open_waits(100, 200).unwrap();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(
        (open[0].count, open[0].asks.as_str()),
        (2, "Bash: git push")
    );
}
