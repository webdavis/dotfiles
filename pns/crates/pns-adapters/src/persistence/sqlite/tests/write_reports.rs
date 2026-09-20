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
        let line = std::fs::read_to_string(&store.log).unwrap();
        assert!(
            line.starts_with(&format!(
                "pns: state error ({operation}: state file unavailable: state file: "
            )) && line.ends_with("); recording failed\n"),
            "{line}"
        );
    }
    assert_eq!(
        std::fs::read(blocked).unwrap(),
        b"unavailable state directory"
    );
}

#[test]
fn a_database_report_names_its_sqlite_code_and_constraint() {
    let path = state();
    let mut store = SqliteStore::new(path.clone());
    store.log = path.join("database.log");
    let refusal = store
        .connect()
        .unwrap()
        .execute("INSERT INTO lamp_news(id, body) VALUES (2, 'body')", [])
        .unwrap_err();
    store.report("lamp news", &refusal.into());
    assert_eq!(
        std::fs::read_to_string(&store.log).unwrap(),
        "pns: state error (lamp news: database refused the operation: state database: \
         CHECK constraint failed: id = 1); recording failed\n"
    );
}

#[test]
fn an_invalid_state_report_names_its_sentence() {
    let path = state();
    std::fs::create_dir(&path).unwrap();
    let mut store = SqliteStore::new(path.clone());
    store.log = path.join("invalid.log");
    store.report(
        "decision",
        &StoreError::InvalidState("a decision line without a legs field".into()),
    );
    assert_eq!(
        std::fs::read_to_string(&store.log).unwrap(),
        "pns: state error (decision: unreadable state record: \
         a decision line without a legs field); recording failed\n"
    );
}
