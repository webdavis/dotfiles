use super::{SqliteStore, StoreError, rows::Ring};
impl SqliteStore {
    pub fn record_presence(&self, entry: &pns_domain::PresenceDecision) -> Result<(), StoreError> {
        self.append(Ring::Presence, &crate::presence_journal::entry(entry))
    }
    pub fn presence_history(&self) -> Result<Option<String>, StoreError> {
        self.read_ring(Ring::Presence)
    }
}
