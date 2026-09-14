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
