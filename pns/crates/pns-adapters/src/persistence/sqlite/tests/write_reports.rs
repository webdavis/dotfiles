use super::{SqliteStore, state};
use crate::StoreError;
use pns_domain::lamps::config::Behaviour;

#[test]
fn direct_record_writes_report_each_failed_store_without_creating_a_legacy_authority() {
    let path = state();
    std::fs::create_dir(&path).unwrap();
    let blocked = path.join("blocked");
    std::fs::write(&blocked, "unavailable state directory").unwrap();
    type RecordWrite = fn(&SqliteStore) -> Result<(), StoreError>;
    let writes: [(&str, RecordWrite); 3] = [
        ("lamp news", |store| {
            store.record_news(Behaviour::Done, Some(1))
        }),
        ("return edge", |store| store.mark_present(1)),
        ("policy settings", |store| {
            store.record_policy_settings_change("session", "private path", Some(1))
        }),
    ];
    for (operation, write) in writes {
        let mut store = SqliteStore::for_records(blocked.clone());
        store.log = path.join(operation);
        assert!(write(&store).is_err());
        assert_eq!(
            std::fs::read_to_string(&store.log).unwrap(),
            format!("pns: state error ({operation}: state file unavailable); recording failed\n")
        );
    }
    assert_eq!(
        std::fs::read(blocked).unwrap(),
        b"unavailable state directory"
    );
}
