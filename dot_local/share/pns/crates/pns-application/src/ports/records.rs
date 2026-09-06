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

use pns_domain::decision::{Decision, Overrides};
use pns_domain::lamps::config::Behaviour;
use pns_domain::missed::Entry;
use pns_domain::notification::EventArgs;

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

/// The lamp records this event writes: what is news, and what is held.
///
/// TWO OPERATIONS AND NOT A READ-THEN-WRITE. `news` decides whether this event
/// is news at all and claims the record in one step, because a caller that
/// could read the last behaviour and then write a new one would race a second
/// event through the gap. `clear_held` puts out every lamp the held file
/// names and works off names alone, so it writes nothing back.
///
/// NO CLOCK IS NO RECORD, never a record at epoch zero: the bound that ages a
/// record out is measured against this number.
///
/// Checked against `record_news` (`src/main.rs:6595`) and `clear_held_lamps`
/// (`src/main.rs:3308`). Statements: S230.
pub trait LampRecords {
    fn news(&self, behaviour: Behaviour, now: Option<u64>);
    fn clear_held(&self);
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

/// The marker behind the blocked lamp: one wait, started and cleared.
///
/// `lamps_live` IS THE CALLER'S ANSWER AND NOT THIS PORT'S. A start is only
/// written where a lamp map and a transport are both live, because a marker
/// written with no lamp to read it is a wait nothing will ever clear; a clear
/// is written regardless, since a marker left behind by a live evening must
/// still be cleared when the lamps go away. Statements: S117.
pub trait BlockedMarker {
    fn update(&self, session_id: &str, event_state: &str, lamps_live: bool, now: Option<u64>);
}

/// The loop lease this pane holds, if it holds one.
///
/// IT RENEWS AND NEVER CREATES. The renewal is the pane's own ordinary
/// traffic, which is what makes the lease a liveness signal rather than a
/// timer; a machine with no lamps pays one failed open and keeps no state.
/// NO CLOCK IS NO RENEWAL, never a renewal at epoch zero.
pub trait LoopLease {
    fn renew(&self, pane: &str, now: Option<u64>);
}

/// This session's nag: the schedule that nudges about a wait nobody answered.
///
/// ARMING IS ONE STEP AND NOT THREE. It unlinks the session's answered marker,
/// publishes the record and re-arms, and a caller that could do two of those
/// would leave a session armed against a marker that says it is already
/// answered. Which agents nag at all, and after how long, is the adapter's
/// read of the config. Statements: S074, S237.
pub trait NagSchedule {
    fn arm(&self, session_id: &str, event: &EventArgs);
}

/// The lights tick this event registers against the job spool.
///
/// ITS OWN PORT BESIDE `JobSpool` RATHER THAN AN OPERATION ON IT. The spool's
/// own vocabulary is a job id and a line, which PR 6.9's daemon speaks; this
/// caller has a decision and a set of overrides and no opinion about either.
/// Which lease a journalled event earns, and what the tick's argv is, are the
/// adapter's.
///
/// NO CLOCK IS NO REGISTRATION, never a job due at epoch zero.
///
/// Checked against `register_lights_tick` (`src/main.rs:3364`). Statements:
/// S231.
pub trait LightsTick {
    fn register(&self, decision: &Decision, overrides: &Overrides);
}
