use super::{decisions, journal};
use crate::{RING_READ_MAX, append_ring_line, readable_state_file};
use pns_application::{ActivityRing, DecisionRing, Journal};
use pns_domain::{EventArgs, Record, missed::Entry};
use std::path::{Path, PathBuf};

/// The three event records stored in one configured state directory.
pub struct FileRecords {
    state: PathBuf,
}

impl FileRecords {
    pub fn new(state: PathBuf) -> Self {
        Self { state }
    }
}

impl DecisionRing for FileRecords {
    fn record(&self, record: &Record) {
        let _ = append_ring_line(
            &self.state.join(DECISIONS),
            &decisions::line(record),
            pns_domain::KEPT,
            RING_READ_MAX,
        );
    }
    fn read(&self) -> Result<Option<String>, String> {
        read_ring(&self.state.join(DECISIONS))
    }
}

impl Journal for FileRecords {
    fn journal(
        &self,
        event: &EventArgs,
        now: Option<u64>,
        _identity: Option<&pns_application::SubmissionIdentity>,
    ) {
        let _ = append_ring_line(
            &self.state.join(MISSED_NOTIFICATIONS),
            &journal::entry(event, now, pns_domain::render::PREVIEW_MAX_CHARS),
            pns_domain::missed::KEPT,
            RING_READ_MAX,
        );
    }
    fn read(&self) -> Result<Option<String>, String> {
        read_ring(&self.state.join(MISSED_NOTIFICATIONS))
    }
}

impl ActivityRing for FileRecords {
    fn record(&self, event: &EventArgs, now: Option<u64>) {
        let _ = append_ring_line(
            &self.state.join(ACTIVITY),
            &journal::entry(event, now, ACTIVITY_MAX_CHARS),
            ACTIVITY_KEPT,
            ACTIVITY_READ_MAX,
        );
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<Entry> {
        let Ok(contents) = readable_state_file(&self.state.join(ACTIVITY), ACTIVITY_READ_MAX)
        else {
            return Vec::new();
        };
        journal::entries(&contents)
            .into_iter()
            .filter(|entry| entry.at.is_some_and(|at| at > since && at <= until))
            .collect()
    }
}

fn read_ring(path: &Path) -> Result<Option<String>, String> {
    readable_state_file(path, RING_READ_MAX)
        .map(Some)
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(error.kind().to_string())
            }
        })
}
/// The most of the ACTIVITY ring that is ever read into memory, which is its
/// own number because its depth is its own.
///
/// THE ARITHMETIC, in `KEPT`'s style so the next person to raise either number
/// has the ceiling in front of them. A worst-case entry is five text fields at
/// `ACTIVITY_MAX_CHARS` characters, each character costing six bytes escaped
/// (a control byte is written `\u001b`), plus about eighty bytes of JSON
/// scaffolding: 5 * 120 * 6 + 80 = 3,680 bytes. At `ACTIVITY_KEPT` that
/// MEASURES 552,000 bytes, which is 53% of this ceiling. Raising the depth or
/// the field cap means raising this in the same change, because a ring that
/// cannot be read back cannot be pruned and collapses to one line.
pub const ACTIVITY_READ_MAX: u64 = 1024 * 1024;
/// The decision ring: one line per event, `KEPT` deep, beside `quiet-until`
/// and `home-staleness`. NOT a log stream and not rotate-logs' business: it is
/// bounded state that prunes itself.
pub const DECISIONS: &str = "decisions";
/// The missed-notification journal: one JSON object per line, oldest first,
/// `missed_notifications::KEPT` deep, beside `decisions` and `quiet-until`.
/// Bounded state that prunes itself, not a log stream and not rotate-logs'
/// business.
pub const MISSED_NOTIFICATIONS: &str = "missed-notifications";
/// The activity ring: EVERY event, one JSON object per line in the journal's
/// own shape, oldest first, `ACTIVITY_KEPT` deep. Bounded state that prunes
/// itself, never claimed and never consumed.
pub const ACTIVITY: &str = "activity";
/// How many events the activity ring keeps.
///
/// A HUNDRED AND FIFTY covers an overnight window at the observed working rate
/// (ten pull requests merged in a ten-hour stretch on 2026-08-29, each spanning
/// many turns and so many events). Past that the ring under-reports its oldest
/// end exactly as the journal's prune does, which is why the recap's header
/// counts the entries it READ rather than claiming a total it cannot back.
/// Raising it means raising `ACTIVITY_READ_MAX` in the same change.
pub const ACTIVITY_KEPT: usize = 150;
/// How much of each text field one activity entry holds.
///
/// A TIMELINE LINE, NOT A CARD, which is why it is far under the card's own
/// 260: the recap renders one line per event among a hundred, and the full text
/// of every event already reached the durable log the recap's tail points at.
pub const ACTIVITY_MAX_CHARS: usize = 120;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doctor_ring_reads_keep_absence_and_error_kind_without_exposing_paths() {
        let state = crate::state_fixtures::scratch("doctor-record-kinds");
        let records = FileRecords::new(state.clone());
        assert_eq!(DecisionRing::read(&records), Ok(None));
        assert_eq!(Journal::read(&records), Ok(None));
        std::fs::create_dir(state.join(DECISIONS)).unwrap();
        std::fs::create_dir(state.join(MISSED_NOTIFICATIONS)).unwrap();
        assert_eq!(
            DecisionRing::read(&records),
            Err(std::io::ErrorKind::InvalidInput.to_string())
        );
        assert_eq!(
            Journal::read(&records),
            Err(std::io::ErrorKind::InvalidInput.to_string())
        );
        assert!(state.join(DECISIONS).is_dir());
        assert!(state.join(MISSED_NOTIFICATIONS).is_dir());
    }
}
