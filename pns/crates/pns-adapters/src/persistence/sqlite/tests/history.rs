use super::{SqliteStore, state};
#[test]
fn presence_history_retains_five_exact_codec_lines_and_the_latest_narrowing() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    let mut expected = Vec::new();
    for at in 0..6 {
        let entry = pns_domain::PresenceDecision {
            at: Some(at),
            presence: "room \"A\"".into(),
            desk_idle_secs: None,
            home: "home".into(),
            room: Some(format!("Room {at}\nannex")),
            reason: String::new(),
        };
        store.record_presence(&entry).unwrap();
        expected.push(crate::presence_journal::entry(&entry));
        if at == 4 {
            assert_eq!(
                store.presence_history().unwrap().unwrap(),
                format!("{}\n", expected.join("\n"))
            );
        }
    }
    let held = store.presence_history().unwrap().unwrap();
    assert_eq!(held, format!("{}\n", expected[1..].join("\n")));
    assert_eq!(
        crate::presence_journal::last(&held)
            .unwrap()
            .room
            .as_deref(),
        Some("Room 5\nannex")
    );
}
#[test]
fn migrating_a_version_13_store_drops_the_policy_audit_table() {
    let state = state();
    let connection = SqliteStore::new(state.clone()).connect().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS policy_audit (seq INTEGER PRIMARY KEY, line TEXT NOT NULL); \
             PRAGMA user_version = 13;",
        )
        .unwrap();
    drop(connection);
    let connection = SqliteStore::new(state).connect().unwrap();
    let left: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'policy_audit'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}
