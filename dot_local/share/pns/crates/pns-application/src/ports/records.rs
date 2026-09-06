//! What the machine remembers between events.
//!
//! SIX PORTS AND NOT ONE, though five of them are the same ring file on disk
//! today. The use case that records a decision and the use case that journals a
//! missed notification are asking for different things, and a single `Ring`
//! port would make every call site name which file it meant. A port is named
//! for the question it answers, so the adapter is where a path, a cap and a
//! retention count live.
//!
//! EVERY WRITE ANSWERS NOTHING. A record nobody could write is dropped at the
//! adapter, because the notification path always exits 0 and a complaint about
//! the state directory in every hook's output is worse than a missing
//! diagnostic. A port that returned a result would offer a decision no caller
//! can act on.

use pns_domain::missed::Entry;

/// The decision log: why a card did or did not fire, newest first.
///
/// Read as ONE STRING rather than parsed rows, because the only reader parses
/// it itself and `None` is a machine that has recorded nothing yet, which is
/// not a failure. Statements: S157.
pub trait DecisionRing {
    fn record(&self, line: &str);
    fn read(&self) -> Option<String>;
}

/// The missed-notification journal: events the operator could not have
/// perceived, kept so a replayer can find them.
pub trait Journal {
    fn journal(&self, entry: &str);
    fn read(&self) -> Option<String>;
}

/// The activity ring: every event, WHETHER OR NOT anybody perceived it, which
/// is what makes it a different record from the journal above. The recap reads
/// this one to say what happened while the operator was away.
pub trait ActivityRing {
    fn record(&self, entry: &str);
    fn read(&self) -> Option<String>;
}

/// The near edge of the recap window, and the journal claimed with it.
///
/// ONE CLAIM, ONE OWNER. Claiming moves the edge and takes the waiting
/// journal in the same critical section, because an event that took the
/// entries without moving the edge would replay them again on the next event.
/// `None` is a claim somebody else holds right now, which silences this event
/// rather than failing it.
pub trait ReturnMoment {
    fn claim(&self, now: Option<u64>) -> Option<Claim>;
}

/// What claiming the moment yielded: the edge the marker held, absent when
/// there was no marker to open a window with, and the journal taken with it.
#[derive(Debug, Default, PartialEq)]
pub struct Claim {
    pub since: Option<u64>,
    pub waiting: Vec<Entry>,
}

/// What the lamps were last told, so a tick that would repeat itself does not
/// spend a bridge call saying it again.
pub trait LampRecords {
    fn last_written(&self) -> Option<String>;
    fn remember(&self, state: &str);
}

/// The daemon's spool of scheduled jobs.
///
/// `claim` IS WHAT MAKES A JOB RUN ONCE. Two daemons reading the same spool
/// both see a job; only the one whose claim succeeds may run it, and on macOS
/// that has to be a rename rather than a delete, because concurrent unlink
/// reports success to every racer on APFS.
pub trait JobSpool {
    fn schedule(&self, id: &str, line: &str, now: u64) -> Result<(), String>;
    fn cancel(&self, id: &str) -> Result<bool, String>;
    fn due(&self, now: u64) -> Vec<String>;
    fn claim(&self, id: &str) -> bool;
}
