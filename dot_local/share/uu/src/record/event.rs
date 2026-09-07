use super::marker::Marker;
use uu_protocol::{LastSuccessfulRun, RunEvent};

/// What a command lane's own run event needs: `main` computes `started` and
/// drops the `Marker` inside `gap_line` before the lane loop runs, so this
/// struct carries those facts into `run_lane` rather than each lane
/// recomputing them.
pub struct RunFacts<'a> {
    pub host: &'a str,
    pub started_epoch: i64,
    pub started_iso: &'a str,
    pub marker: &'a Marker,
}

impl<'a> From<&'a RunFacts<'_>> for RunEvent<'a> {
    fn from(facts: &'a RunFacts<'_>) -> Self {
        Self {
            host: facts.host,
            started_epoch: facts.started_epoch,
            started_iso: facts.started_iso,
            marker: match facts.marker {
                Marker::NeverRecorded => LastSuccessfulRun::NeverRecorded,
                Marker::Unreadable => LastSuccessfulRun::Unreadable,
                Marker::Recorded { epoch, iso } => {
                    LastSuccessfulRun::Recorded { epoch: *epoch, iso }
                }
            },
        }
    }
}
