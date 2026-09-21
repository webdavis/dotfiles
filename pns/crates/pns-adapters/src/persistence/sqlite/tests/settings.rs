use super::{SqliteStore, state};
#[test]
fn quiet_expiry_and_staleness_survive_reopening_and_clear_independently() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    assert_eq!(store.mute_expiry().unwrap(), None);
    assert_eq!(store.staleness().unwrap(), None);
    store.set_mute_expiry(Some(i64::MAX as u64)).unwrap();
    store.remember_staleness(Some("episode A")).unwrap();
    let reopened = SqliteStore::new(path);
    assert_eq!(reopened.mute_expiry().unwrap(), Some(i64::MAX as u64));
    assert_eq!(reopened.staleness().unwrap().as_deref(), Some("episode A"));
    reopened.set_mute_expiry(None).unwrap();
    assert_eq!(reopened.mute_expiry().unwrap(), None);
    assert_eq!(reopened.staleness().unwrap().as_deref(), Some("episode A"));
    reopened.remember_staleness(None).unwrap();
    assert_eq!(reopened.staleness().unwrap(), None);
}
#[test]
fn the_two_lamp_complaint_memories_never_overwrite_or_forget_each_other() {
    let store = SqliteStore::new(state());
    store
        .remember_lights_complaint(Some("tick refusal"))
        .unwrap();
    store
        .remember_quiet_complaint(Some("quiet refusal"))
        .unwrap();
    assert_eq!(
        store.lights_complaint().unwrap().as_deref(),
        Some("tick refusal")
    );
    assert_eq!(
        store.quiet_complaint().unwrap().as_deref(),
        Some("quiet refusal")
    );
    store.remember_quiet_complaint(None).unwrap();
    assert_eq!(store.quiet_complaint().unwrap(), None);
    assert_eq!(
        store.lights_complaint().unwrap().as_deref(),
        Some("tick refusal")
    );
}
#[test]
fn a_busy_setting_change_reports_failure_and_keeps_the_previous_expiry() {
    let mut store = SqliteStore::new(state());
    store.busy_timeout = std::time::Duration::from_millis(5);
    store.set_mute_expiry(Some(7)).unwrap();
    let mut connection = store.connect().unwrap();
    let _held = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    assert!(store.set_mute_expiry(Some(9)).is_err());
    assert_eq!(store.mute_expiry().unwrap(), Some(7));
}
#[test]
fn the_configured_busy_deadline_reaches_the_connection_that_waits_on_the_lock() {
    // THE WHOLE PATH IN ONE ASSERTION: `[storage] busy_deadline` parses, the
    // install settings carry it, and the connection's own `busy_timeout`
    // pragma answers with it in milliseconds. The bound used to come off
    // `PNS_DB_BUSY_TIMEOUT_MS`, which production code read on every
    // connection, so a stray variable on a real machine decided how long a
    // wedged writer was waited on.
    let config = crate::parse_config("[storage]\nbusy_deadline = \"250ms\"\n").expect("the config");
    let settings = crate::install_settings_of(Some(&config), "/nonexistent-home");
    let mut store = SqliteStore::new(state());
    store.busy_timeout = settings.busy_deadline;
    let connection = store.connect().expect("the database");
    assert_eq!(
        connection
            .pragma_query_value(None, "busy_timeout", |row| row.get::<_, i64>(0))
            .expect("the connection's busy bound"),
        250
    );
}
