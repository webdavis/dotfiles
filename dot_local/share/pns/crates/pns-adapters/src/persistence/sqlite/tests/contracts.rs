use super::{SqliteStore, state};
use pns_application::{ActivityRing, DecisionRing, Journal};
use pns_domain::{EventArgs, missed::Entry};
use std::cell::RefCell;

#[derive(Default)]
struct MemoryRecords {
    decisions: RefCell<Vec<String>>,
    journal: RefCell<Vec<String>>,
    activity: RefCell<Vec<Entry>>,
}
impl Journal for MemoryRecords {
    fn journal(
        &self,
        event: &EventArgs,
        now: Option<u64>,
        _identity: Option<&pns_application::SubmissionIdentity>,
    ) {
        let mut rows = self.journal.borrow_mut();
        rows.push(crate::journal_codec::entry(event, now, 260));
        if rows.len() > 25 {
            rows.remove(0);
        }
    }
    fn read(&self) -> Result<Option<String>, String> {
        let rows = self.journal.borrow();
        Ok((!rows.is_empty()).then(|| format!("{}\n", rows.join("\n"))))
    }
}
impl ActivityRing for MemoryRecords {
    fn record(&self, event: &EventArgs, now: Option<u64>) {
        let mut rows = self.activity.borrow_mut();
        rows.extend(crate::journal_codec::entries(&crate::journal_codec::entry(
            event, now, 120,
        )));
        if rows.len() > 150 {
            rows.remove(0);
        }
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<Entry> {
        self.activity
            .borrow()
            .iter()
            .filter(|entry| entry.at.is_some_and(|at| at > since && at <= until))
            .cloned()
            .collect()
    }
}
fn event(detail: String) -> EventArgs {
    EventArgs {
        agent: "actor".into(),
        state: "done".into(),
        project: "workspace".into(),
        branch: "topic".into(),
        detail,
        ..EventArgs::default()
    }
}
fn journal_retention(records: &impl Journal) {
    assert_eq!(records.read().unwrap(), None);
    for n in 0..25 {
        records.journal(&event(n.to_string()), Some(n), None);
    }
    let first = records.read().unwrap().unwrap();
    let first = crate::journal_codec::entries(&first);
    assert_eq!(first.len(), 25);
    assert_eq!(first.first().unwrap().detail, "0");
    records.journal(&event("25".into()), Some(25), None);
    let pruned = crate::journal_codec::entries(&records.read().unwrap().unwrap());
    assert_eq!(pruned.len(), 25);
    assert_eq!(pruned.first().unwrap().detail, "1");
    assert_eq!(pruned.last().unwrap().detail, "25");
}
fn activity_window(records: &impl ActivityRing) {
    for (detail, at) in [
        ("before", Some(9)),
        ("near", Some(10)),
        ("inside", Some(11)),
        ("far", Some(12)),
        ("after", Some(13)),
        ("unknown", None),
        ("maximum", Some(u64::MAX)),
    ] {
        records.record(&event(detail.into()), at);
    }
    assert_eq!(
        records
            .entries_between(10, 12)
            .iter()
            .map(|row| row.detail.as_str())
            .collect::<Vec<_>>(),
        ["inside", "far"]
    );
    assert!(records.entries_between(12, 10).is_empty());
    assert_eq!(
        records.entries_between(u64::MAX - 1, u64::MAX)[0].detail,
        "maximum"
    );
}
#[test]
fn the_sqlite_journal_keeps_twenty_five_entries_in_arrival_order() {
    let store = SqliteStore::new(state());
    let _keep_wal_open = store.connect().unwrap();
    journal_retention(&store);
}
#[test]
fn the_memory_journal_runs_the_same_retention_contract() {
    journal_retention(&MemoryRecords::default());
}
#[test]
fn the_sqlite_activity_window_excludes_the_near_edge_and_unknown_clocks() {
    let store = SqliteStore::new(state());
    let _keep_wal_open = store.connect().unwrap();
    activity_window(&store);
}
#[test]
fn the_memory_activity_runs_the_same_window_contract() {
    activity_window(&MemoryRecords::default());
}

impl DecisionRing for MemoryRecords {
    fn record(&self, record: &pns_domain::Record<'_>) {
        let mut rows = self.decisions.borrow_mut();
        rows.push(crate::decision_codec::line(record));
        if rows.len() > 5 {
            rows.remove(0);
        }
    }
    fn read(&self) -> Result<Option<String>, String> {
        let rows = self.decisions.borrow();
        Ok((!rows.is_empty()).then(|| format!("{}\n", rows.join("\n"))))
    }
}
fn decision_retention(records: &impl DecisionRing) {
    let event = event("private detail must not reach decision history".into());
    let overrides = pns_domain::Overrides::default();
    let selection = pns_domain::registry::Registry::new().all();
    let mut expected = Vec::new();
    assert_eq!(records.read().unwrap(), None);
    for now in 0..6 {
        let decision = pns_domain::decide(
            &pns_domain::EnvironmentSnapshot::default(),
            &selection,
            &overrides,
            pns_domain::DecisionRequest {
                silence_policy: pns_domain::SilencePolicy::Respect,
                scope: pns_domain::DeliveryScope::Automatic,
                pane: "",
                now_secs: Some(now),
                long_running: false,
                mobile_watch_card: false,
            },
        );
        let record = pns_domain::Record {
            event: &event,
            decision: &decision,
            overrides: &overrides,
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        };
        expected.push(crate::decision_codec::line(&record));
        records.record(&record);
        if now == 4 {
            assert_eq!(
                records.read().unwrap().unwrap(),
                format!("{}\n", expected.join("\n"))
            );
        }
    }
    let held = records.read().unwrap().unwrap();
    assert_eq!(held, format!("{}\n", expected[1..].join("\n")));
    assert!(!held.contains("private detail"));
}
fn activity_retention(records: &impl ActivityRing) {
    for now in 1..=150 {
        records.record(&event(now.to_string()), Some(now));
    }
    assert_activity_retention(records);
}
fn assert_activity_retention(records: &impl ActivityRing) {
    let before = records.entries_between(0, 151);
    assert_eq!(before.len(), 150);
    assert_eq!(before[0].detail, "1");
    records.record(&event("151".into()), Some(151));
    let after = records.entries_between(0, 151);
    assert_eq!(after.len(), 150);
    assert_eq!(after[0].detail, "2");
    assert_eq!(after[149].detail, "151");
}
#[test]
fn the_sqlite_decision_history_keeps_five_exact_private_codec_records() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    decision_retention(&store);
}
#[test]
fn the_memory_decisions_run_the_same_retention_and_privacy_contract() {
    decision_retention(&MemoryRecords::default());
}
#[test]
fn the_sqlite_activity_ring_prunes_only_the_one_hundred_fifty_first_entry() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    store
        .transaction(|transaction| {
            for now in 1..150 {
                let line = crate::journal_codec::entry(&event(now.to_string()), Some(now), 120);
                transaction.execute(
                    "INSERT INTO activity(line) VALUES (?1)",
                    [format!("{line}\n")],
                )?;
            }
            Ok(())
        })
        .unwrap();
    ActivityRing::record(&store, &event("150".into()), Some(150));
    assert_activity_retention(&store);
}
#[test]
fn the_memory_activity_runs_the_same_one_hundred_fifty_entry_contract() {
    activity_retention(&MemoryRecords::default());
}
