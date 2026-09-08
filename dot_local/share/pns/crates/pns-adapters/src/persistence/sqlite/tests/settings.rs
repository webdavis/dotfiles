use super::{SqliteStore, state};
#[test]
fn quiet_expiry_and_staleness_survive_reopening_and_clear_independently() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    assert_eq!(store.quiet_expiry().unwrap(), None);
    assert_eq!(store.staleness().unwrap(), None);
    store.set_quiet_expiry(Some(i64::MAX as u64)).unwrap();
    store.remember_staleness(Some("episode A")).unwrap();
    let reopened = SqliteStore::new(path);
    assert_eq!(reopened.quiet_expiry().unwrap(), Some(i64::MAX as u64));
    assert_eq!(reopened.staleness().unwrap().as_deref(), Some("episode A"));
    reopened.set_quiet_expiry(None).unwrap();
    assert_eq!(reopened.quiet_expiry().unwrap(), None);
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
    store.set_quiet_expiry(Some(7)).unwrap();
    let mut connection = store.connect().unwrap();
    let _held = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    assert!(store.set_quiet_expiry(Some(9)).is_err());
    assert_eq!(store.quiet_expiry().unwrap(), Some(7));
}
