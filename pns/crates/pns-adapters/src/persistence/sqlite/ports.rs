use super::{SqliteStore, scalar::Scalar};
use pns_application::{
    HeldLamps, LampComplaint, LampComplaints, LampHouseRecords, LampMutes, PresenceDecisions,
    StalenessMemory,
};
use pns_domain::lights::{mute::Muted, phase::HeldEntry, streak::Streak, unread::News};
impl HeldLamps for SqliteStore {
    fn read(&self) -> Option<Vec<HeldEntry>> {
        self.read_held().ok()
    }
    fn remember(&self, entries: &[HeldEntry]) -> Result<(), String> {
        self.remember_held(entries)
            .map_err(|error| error.to_string())
    }
}
impl LampMutes for SqliteStore {
    fn read(&self) -> (Vec<Muted>, Vec<String>) {
        match self.read_muted() {
            Ok(entries) => (entries, Vec::new()),
            Err(super::StoreError::InvalidState(complaint)) => {
                let said = complaint.strip_suffix("replaces the file").map_or_else(
                    || complaint.clone(),
                    |prefix| format!("{prefix}replaces the stored record"),
                );
                (Vec::new(), vec![said])
            }
            Err(error) => (
                Vec::new(),
                vec![format!(
                    "pns: state error (lights-quiet could not be read: {error}); nothing is muted"
                )],
            ),
        }
    }
    fn write(&self, entries: &[Muted]) -> Result<(), String> {
        self.write_muted(entries).map_err(|error| error.to_string())
    }
}
impl LampHouseRecords for SqliteStore {
    fn advance_streak(&self, working: bool, now: u64) -> Option<Streak> {
        match self.advance_streak(working, now) {
            Ok(next) => next,
            Err(error) => {
                self.report("working streak", &error);
                // The legacy caller still receives the computed next streak
                // when publication fails. This read-only fallback never retries
                // the mutation or claims that the returned value was stored.
                let held = self
                    .connect()
                    .ok()
                    .and_then(|connection| Scalar::Streak.stored(&connection).ok().flatten())
                    .as_deref()
                    .and_then(crate::lights_codec::parse_streak);
                pns_domain::lights::streak::next_streak(
                    held,
                    working,
                    now,
                    super::lamps::WORKING_GRACE_SECS,
                )
            }
        }
    }
    fn news(&self) -> News {
        self.read_news().unwrap_or_default()
    }
}
impl LampComplaints for SqliteStore {
    fn remembered(&self, kind: LampComplaint) -> String {
        match kind {
            LampComplaint::Tick => self.lights_complaint(),
            LampComplaint::Quiet => self.quiet_complaint(),
        }
        .ok()
        .flatten()
        .unwrap_or_default()
    }
    fn remember(&self, kind: LampComplaint, said: Option<&str>) {
        let result = match kind {
            LampComplaint::Tick => self.remember_lights_complaint(said),
            LampComplaint::Quiet => self.remember_quiet_complaint(said),
        };
        if let Err(error) = result {
            self.report("lamp complaint", &error);
        }
    }
}
impl StalenessMemory for SqliteStore {
    fn remembered(&self) -> Option<String> {
        self.staleness().ok().flatten()
    }
    fn remember(&self, episode: Option<&str>) {
        if let Err(error) = self.remember_staleness(episode) {
            self.report("home staleness", &error);
        }
    }
}
impl PresenceDecisions for SqliteStore {
    fn record(&self, snapshot: &pns_domain::Snapshot, decision: &pns_domain::Narrowing) {
        if let Err(error) =
            self.record_presence(&crate::presence_journal::recorded(snapshot, decision))
        {
            self.report("presence decision", &error);
        }
    }
}
impl pns_application::SessionWaits for SqliteStore {
    fn begin(&self, session_id: &str, now: u64) -> Result<(), String> {
        self.begin_wait(session_id, now)
            .map_err(|error| error.to_string())
    }
    fn end(&self, session_id: &str) -> Result<(), String> {
        self.end_wait(session_id).map_err(|error| error.to_string())
    }
}
impl pns_application::StaleWaits for SqliteStore {
    /// A READ NOBODY CAN TAKE IS NO STALE BLOCK, said out loud and never
    /// silently: the fire is unattended, so the one reader it has is the
    /// daemon's log.
    fn waiting_since(&self, threshold: u64) -> Vec<pns_domain::stale::Blocked> {
        match self.stale_blocks(threshold) {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!(
                    "pns: state error (the waiting sessions could not be read: {error}); \
                     nothing is escalated"
                );
                Vec::new()
            }
        }
    }
    /// AN UNWRITABLE CLAIM IS NOT A CLAIM, so the page is not sent: a page
    /// nothing stamped would be sent again by every later fire.
    fn claim(&self, session_id: &str, now: u64) -> bool {
        match self.claim_escalation(session_id, now) {
            Ok(claimed) => claimed,
            Err(error) => {
                eprintln!(
                    "pns: state error (this escalation could not be stamped: {error}); \
                     it is not paged"
                );
                false
            }
        }
    }
}
