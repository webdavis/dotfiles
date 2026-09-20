use super::{SqliteStore, migrations, state};
use crate::persistence::sqlite::ActivityEvent;

fn event(at: u64) -> ActivityEvent {
    ActivityEvent {
        at,
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        project: "dotfiles".to_string(),
        branch: "feat/pns-activity".to_string(),
        session: "s1".to_string(),
        session_title: "the activity table".to_string(),
        pane: "w1:p2".to_string(),
        workspace: "w1".to_string(),
        model: "claude-opus-5".to_string(),
        title: "claude blocked in dotfiles".to_string(),
        detail: "Bash(rm -rf build)".to_string(),
    }
}

#[test]
fn one_event_is_one_row_carrying_every_field_the_recap_reads() {
    let store = SqliteStore::new(state());
    let written = event(1_700_000_000);
    store.record_activity_event(&written).unwrap();
    assert_eq!(
        store.activity_between(0, 1_700_000_001).unwrap(),
        vec![written],
        "every column goes in and comes back, including the ones the hook \
         reads off the transcript and the environment"
    );
}

#[test]
fn a_row_older_than_the_retention_is_pruned_and_a_younger_one_stays() {
    // THE CUTOFF IS THE CALLER'S `now - retain`, and it is STRICT, so a row
    // written exactly one retention ago survives the sweep it sits on.
    let store = SqliteStore::new(state());
    for at in [900, 1_000, 1_100] {
        store.record_activity_event(&event(at)).unwrap();
    }
    assert_eq!(store.prune_activity(1_000).unwrap(), 1);
    assert_eq!(
        store
            .activity_between(0, 2_000)
            .unwrap()
            .into_iter()
            .map(|event| event.at)
            .collect::<Vec<_>>(),
        vec![1_000, 1_100]
    );
}

#[test]
fn a_store_on_the_previous_schema_migrates_once_and_reopening_it_changes_nothing() {
    // THE SHAPE A RUNNING MACHINE HAS on the first start after the apply: a
    // version 10 database with no activity table. The second open is what
    // pins idempotence, since a migration that ran twice would fail on the
    // CREATE.
    let state = state();
    let store = SqliteStore::new(state.clone());
    let connection = store.connect().unwrap();
    connection
        .execute_batch("DROP TABLE activity_events; PRAGMA user_version = 10;")
        .unwrap();
    drop(connection);

    for pass in 1..=2 {
        let connection = SqliteStore::new(state.clone())
            .connect()
            .unwrap_or_else(|error| panic!("pass {pass} migrates: {error}"));
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, migrations::VERSION);
    }
    SqliteStore::new(state)
        .record_activity_event(&event(10))
        .expect("the migrated table takes rows");
}

#[test]
fn a_store_from_a_later_schema_is_refused_rather_than_read() {
    // A RECAP WITH NO AGENTS SECTION IS NOT A RECAP, so a store this build
    // cannot open is a refusal rather than an empty answer.
    let state = state();
    let store = SqliteStore::new(state.clone());
    let connection = store.connect().unwrap();
    connection
        .pragma_update(None, "user_version", migrations::VERSION + 1)
        .unwrap();
    drop(connection);
    let refusal = SqliteStore::new(state)
        .record_activity_event(&event(10))
        .expect_err("a newer schema is refused");
    assert!(
        refusal.to_string().contains("schema"),
        "the refusal names the schema: {refusal}"
    );
}
