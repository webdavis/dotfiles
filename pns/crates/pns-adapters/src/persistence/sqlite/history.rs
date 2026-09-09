use super::{SqliteStore, StoreError, rows::Ring};
impl SqliteStore {
    pub fn record_presence(&self, entry: &pns_domain::PresenceDecision) -> Result<(), StoreError> {
        self.append(Ring::Presence, &crate::presence_journal::entry(entry))
    }
    pub fn presence_history(&self) -> Result<Option<String>, StoreError> {
        self.read_ring(Ring::Presence)
    }
    pub fn record_policy_settings_change(
        &self,
        session: &str,
        path: &str,
        now: Option<u64>,
    ) -> Result<(), StoreError> {
        let now = now.unwrap_or_default();
        let path = if path.is_empty() { "none" } else { path };
        self.append(
            Ring::PolicyAudit,
            &format!("{now} session={session} file={path}"),
        )
        .inspect_err(|error| self.report("policy settings", error))
    }
    pub fn policy_settings_history(&self) -> Result<Option<String>, StoreError> {
        self.read_ring(Ring::PolicyAudit)
    }
}
