use super::{SqliteStore, StoreError, rows::Ring};
use pns_application::{ActivityRing, DecisionRing, Journal};
use pns_domain::{EventArgs, Record, missed::Entry};

impl SqliteStore {
    pub fn record_decision(&self, record: &Record) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            super::decisions::append(transaction, &crate::decision_codec::line(record), None)
        })
    }
    pub fn record_journal(
        &self,
        event: &EventArgs,
        now: Option<u64>,
        _identity: Option<&pns_application::SubmissionIdentity>,
    ) -> Result<(), StoreError> {
        self.transaction(|transaction| super::journal::append(transaction, event, now, _identity))
    }
    pub fn record_activity(&self, event: &EventArgs, now: Option<u64>) -> Result<(), StoreError> {
        self.append(
            Ring::Activity,
            &crate::journal_codec::entry(event, now, crate::ACTIVITY_MAX_CHARS),
        )
    }
}
impl DecisionRing for SqliteStore {
    fn record(&self, record: &Record) {
        if let Err(error) = self.record_decision(record) {
            self.report("decision", &error);
        }
    }
    fn read(&self) -> Result<Option<String>, String> {
        self.read_ring(Ring::Decisions)
            .map_err(|error| error.to_string())
    }
}
impl Journal for SqliteStore {
    fn journal(
        &self,
        event: &EventArgs,
        now: Option<u64>,
        _identity: Option<&pns_application::SubmissionIdentity>,
    ) {
        if let Err(error) = self.record_journal(event, now, _identity) {
            self.report("journal", &error);
        }
    }
    fn read(&self) -> Result<Option<String>, String> {
        self.read_ring(Ring::Journal)
            .map_err(|error| error.to_string())
    }
}
impl ActivityRing for SqliteStore {
    fn record(&self, event: &EventArgs, now: Option<u64>) {
        if let Err(error) = self.record_activity(event, now) {
            self.report("activity", &error);
        }
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<Entry> {
        let Ok(Some(contents)) = self.read_ring(Ring::Activity) else {
            return Vec::new();
        };
        crate::journal_codec::entries(&contents)
            .into_iter()
            .filter(|entry| entry.at.is_some_and(|at| at > since && at <= until))
            .collect()
    }
}
