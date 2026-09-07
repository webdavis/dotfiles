use uu_domain::{Marker, RunFacts};
use uu_protocol::{LastSuccessfulRun, RunEvent};

pub fn event_for<'a>(facts: &'a RunFacts<'_>) -> RunEvent<'a> {
    RunEvent {
        host: facts.host,
        started_epoch: facts.started_epoch,
        started_iso: facts.started_iso,
        marker: match facts.marker {
            Marker::NeverRecorded => LastSuccessfulRun::NeverRecorded,
            Marker::Unreadable => LastSuccessfulRun::Unreadable,
            Marker::Recorded { epoch, iso } => LastSuccessfulRun::Recorded { epoch: *epoch, iso },
        },
    }
}
