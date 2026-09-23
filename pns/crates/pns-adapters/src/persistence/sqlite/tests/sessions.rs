use super::{SqliteStore, state};
use crate::persistence::sqlite::SessionNote;

fn note<'a>(id: &'a str, title: &'a str, now: u64) -> SessionNote<'a> {
    SessionNote {
        id,
        harness: "claude",
        project: "dotfiles",
        branch: "feat/sender-header",
        title,
        now,
    }
}

#[test]
fn the_first_prompt_names_the_session_and_a_later_one_does_not_rename_it() {
    let store = SqliteStore::new(state());
    assert_eq!(
        store
            .note_session(&note("s1", "arm posture alert", 10))
            .unwrap(),
        "arm posture alert"
    );
    assert_eq!(
        store
            .note_session(&note("s1", "and now something unrelated", 20))
            .unwrap(),
        "arm posture alert",
        "the FIRST prompt of a session is the one that names it"
    );
}

#[test]
fn an_event_with_nothing_new_to_say_keeps_what_the_row_already_knows() {
    // The events after the prompt hook carry no title of their own, and the
    // prompt hook itself carries no repository, so an empty value is a
    // caller with nothing to say rather than an erasure.
    let store = SqliteStore::new(state());
    store
        .note_session(&note("s1", "arm posture alert", 10))
        .unwrap();
    let quiet = SessionNote {
        id: "s1",
        harness: "",
        project: "",
        branch: "",
        title: "",
        now: 20,
    };
    assert_eq!(store.note_session(&quiet).unwrap(), "arm posture alert");
    let row: (String, String, String) = SqliteStore::new(store.state.clone())
        .connect()
        .unwrap()
        .query_row(
            "SELECT harness, project, branch FROM sessions WHERE id = 's1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            "claude".to_string(),
            "dotfiles".to_string(),
            "feat/sender-header".to_string()
        )
    );
}

// --- the stale-block escalation ---------------------------------------------

/// An hour, which is the shipped window.
const WINDOW: u64 = 3_600;

#[test]
fn a_block_as_old_as_the_window_is_selected_and_one_second_short_of_it_is_not() {
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.note_session(&note("s2", "two", 0)).unwrap();
    // One wait a whole window old, and one a second short of it.
    let now = 1_000 + WINDOW;
    store.begin_wait("s1", 1_000, true).unwrap();
    store.begin_wait("s2", 1_001, true).unwrap();
    let stale = store.stale_blocks(now - WINDOW).unwrap();
    assert_eq!(
        stale
            .iter()
            .map(|row| row.session.as_str())
            .collect::<Vec<_>>(),
        ["s1"],
        "the window's own second fires and one short of it waits"
    );
    assert_eq!(stale[0].since, 1_000);
    assert_eq!(stale[0].project, "dotfiles");
}

#[test]
fn the_escalation_is_claimed_once_so_a_later_tick_finds_nothing() {
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.begin_wait("s1", 1_000, true).unwrap();
    assert!(store.claim_escalation("s1", 5_000).unwrap());
    assert!(
        !store.claim_escalation("s1", 6_000).unwrap(),
        "the row is stamped, so a second claimant is refused"
    );
    assert!(store.stale_blocks(1_000).unwrap().is_empty());
}

#[test]
fn a_wait_that_ended_is_selected_again_once_a_new_block_starts() {
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.begin_wait("s1", 1_000, true).unwrap();
    store.claim_escalation("s1", 5_000).unwrap();
    store.end_wait("s1", Some(5_000)).unwrap();
    assert!(store.stale_blocks(9_000).unwrap().is_empty());
    store.begin_wait("s1", 9_000, true).unwrap();
    assert_eq!(
        store
            .stale_blocks(9_000)
            .unwrap()
            .iter()
            .map(|row| row.session.as_str())
            .collect::<Vec<_>>(),
        ["s1"],
        "a new block is a new escalation"
    );
}

#[test]
fn a_new_block_is_selected_again_even_where_no_event_ended_the_last_one() {
    // ONE PAGE PER BLOCK, and a block that was never explicitly ended is still
    // over once a new wait starts: `resolved` skips a subagent's batch and a
    // payload past the harness cap never arrives, so a session can reach its
    // next approval with the previous escalation still stamped. Without the
    // clear on the way in, that session would never be escalated again.
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.begin_wait("s1", 1_000, true).unwrap();
    assert!(store.claim_escalation("s1", 5_000).unwrap());
    store.begin_wait("s1", 9_000, true).unwrap();
    assert_eq!(
        store
            .stale_blocks(9_000)
            .unwrap()
            .iter()
            .map(|row| row.session.as_str())
            .collect::<Vec<_>>(),
        ["s1"]
    );
}

#[test]
fn a_wait_begun_with_the_escalation_off_is_never_paged_about() {
    // BORN CLAIMED, so switching the escalation on later pages only the waits
    // begun after it, never a backlog of ones it was never armed for.
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.begin_wait("s1", 1_000, false).unwrap();
    assert!(store.stale_blocks(9_000).unwrap().is_empty());
    assert!(!store.claim_escalation("s1", 9_000).unwrap());
    assert_eq!(
        store.newest_wait().map(|wait| wait.since),
        Some(1_000),
        "the wait itself is still recorded"
    );
}

#[test]
fn a_late_clear_leaves_a_wait_begun_after_its_own_moment() {
    // THE ANSWER ARMS ARE ASYNC, so one batch's clear can land after the
    // next approval began its wait, and must not take it.
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.begin_wait("s1", 2_000, true).unwrap();
    store.end_wait("s1", Some(1_999)).unwrap();
    assert_eq!(store.newest_wait().map(|wait| wait.since), Some(2_000));
    store.end_wait("s1", Some(2_000)).unwrap();
    assert!(store.newest_wait().is_none());
}

#[test]
fn a_clear_with_no_clock_ends_the_wait() {
    let store = SqliteStore::new(state());
    store.note_session(&note("s1", "one", 0)).unwrap();
    store.begin_wait("s1", 2_000, true).unwrap();
    store.end_wait("s1", None).unwrap();
    assert!(store.newest_wait().is_none());
}
