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
#[test]
fn migrating_a_version_13_store_with_no_stale_policy_settings_audit_import_row_still_migrates() {
    // THE ABSENT CASE: a store that never failed to import the retired family
    // has no row to delete, and the migration's DELETE must be a no-op rather
    // than an error.
    let state = state();
    let connection = SqliteStore::new(state.clone()).connect().unwrap();
    connection
        .execute_batch("PRAGMA user_version = 13;")
        .unwrap();
    drop(connection);
    let connection = SqliteStore::new(state).connect().unwrap();
    let left: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM legacy_imports WHERE family = 'policy-settings-audit'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}
#[test]
fn migrating_a_pre_bootstrap_store_with_no_legacy_imports_table_still_migrates() {
    // THE OLDEST CASE: a database that never ran the version-0 bootstrap (a
    // hand-rolled fixture here, an install that predates `legacy_imports`
    // itself in practice) has no such table at all. The delete is guarded
    // rather than assumed, the same way `DROP TABLE IF EXISTS` guards the
    // ring drop beside it.
    let path = state();
    std::fs::create_dir_all(&path).unwrap();
    let connection = rusqlite::Connection::open(path.join("pns.db")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path.join("pns.db"), std::fs::Permissions::from_mode(0o600)).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE return_claims(id INTEGER PRIMARY KEY AUTOINCREMENT, owner INTEGER NOT NULL); \
             CREATE TABLE journal(seq INTEGER PRIMARY KEY, line TEXT NOT NULL, claim INTEGER REFERENCES return_claims(id)); \
             CREATE TABLE decisions(seq INTEGER PRIMARY KEY, line TEXT NOT NULL); \
             PRAGMA user_version = 1;",
        )
        .unwrap();
    drop(connection);
    let migrated: u32 = SqliteStore::new(path)
        .connect()
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(migrated, crate::persistence::sqlite::migrations::VERSION);
}
#[test]
fn migrating_a_version_13_store_drops_the_stale_policy_settings_audit_import_row() {
    // THE PRESENT CASE: a machine whose legacy `policy-settings-audit` file
    // once failed import validation keeps a permanent `legacy_imports` row
    // for a family the table drop above just retired. Nothing can ever clear
    // it again, so the migration removes the bookkeeping row with the table.
    let state = state();
    let connection = SqliteStore::new(state.clone()).connect().unwrap();
    connection
        .execute_batch(
            "INSERT INTO legacy_imports(family, error) VALUES ('policy-settings-audit', 'stale error'); \
             PRAGMA user_version = 13;",
        )
        .unwrap();
    drop(connection);
    let connection = SqliteStore::new(state).connect().unwrap();
    let left: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM legacy_imports WHERE family = 'policy-settings-audit'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}
