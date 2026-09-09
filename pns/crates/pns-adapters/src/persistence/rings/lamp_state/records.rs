use pns_application::LampMutes;
use pns_domain::lights::mute::Muted;
use std::path::PathBuf;

pub struct FileLampState {
    pub(super) state: PathBuf,
}
impl FileLampState {
    pub fn new(state: PathBuf) -> Self {
        Self { state }
    }
}
impl LampMutes for FileLampState {
    fn read(&self) -> (Vec<Muted>, Vec<String>) {
        super::muted_state(&self.state)
    }
    fn write(&self, entries: &[Muted]) -> Result<(), String> {
        super::publish_muted(&self.state.join(super::LIGHTS_QUIET), entries)
            .map_err(|error| error.to_string())
    }
}

impl pns_application::HeldLamps for FileLampState {
    fn read(&self) -> Option<Vec<pns_domain::lights::phase::HeldEntry>> {
        super::read_held(&self.state)
    }
    fn remember(&self, entries: &[pns_domain::lights::phase::HeldEntry]) -> Result<(), String> {
        super::remember_held(&self.state, entries).map_err(|error| error.to_string())
    }
}
impl pns_application::PresenceDecisions for FileLampState {
    fn record(&self, snapshot: &pns_domain::Snapshot, decision: &pns_domain::Narrowing) {
        let _ = crate::append_ring_line(
            &self.state.join("presence-decisions"),
            &crate::presence_journal::entry(&crate::presence_journal::recorded(snapshot, decision)),
            pns_domain::KEPT,
            crate::RING_READ_MAX,
        );
    }
}

impl pns_application::LampHouseRecords for FileLampState {
    fn advance_streak(
        &self,
        working: bool,
        now: u64,
    ) -> Option<pns_domain::lights::streak::Streak> {
        super::advance_streak(&self.state, working, now)
    }
    fn news(&self) -> pns_domain::lights::unread::News {
        super::read_news(&self.state)
    }
}

#[cfg(test)]
#[path = "records_tests.rs"]
mod records_tests;
