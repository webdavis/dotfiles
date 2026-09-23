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
    let writes: [(&str, RecordWrite); 2] = [
        ("lamp news", |store| {
            store.record_news(Behaviour::Done, Some(1))
        }),
        ("return edge", |store| store.mark_present(1)),
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
fn a_database_report_names_its_constraint() {
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

#[test]
fn a_store_on_a_temporary_root_writes_its_diagnostics_inside_that_root() {
    let path = state();
    std::fs::create_dir(&path).unwrap();
    let store = SqliteStore::new(path.clone());
    store.report("decision", &StoreError::InvalidState("provoked".into()));
    assert_eq!(
        std::fs::read_to_string(path.join("pns-daemon.log")).unwrap(),
        "pns: state error (decision: unreadable state record: provoked); recording failed\n"
    );
}

#[test]
fn the_daemon_layout_keeps_its_log_where_the_launch_agent_writes() {
    assert_eq!(
        SqliteStore::new(std::path::PathBuf::from("/stub-home/.local/state/pns")).log,
        std::path::PathBuf::from("/stub-home/.local/log/pns-daemon.log")
    );
}
