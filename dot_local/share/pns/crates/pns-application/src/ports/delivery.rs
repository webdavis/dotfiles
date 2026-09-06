//! Carrying a signal outward: the destinations, and the approval round trip.

use pns_domain::lamps::config::Behaviour;
use pns_domain::notification::Event;
use pns_domain::presence::narrowing::Snapshot;
use pns_domain::routing::{Delivery, ReportMode};

/// One destination a rendered event can reach.
///
/// THE VERDICT IS THE RETURN VALUE, never an error. A destination decides HOW
/// to deliver and whether it can, never WHETHER it should fire, and it must
/// never fail its caller: an always-exit-0 notification path cannot afford a
/// destination that propagates. `Delivery` is what keeps "it arrived", "it did
/// not" and "it was never launched" apart for the one caller that decides
/// whether a line reaches the operator.
///
/// THE MODE IS PER-LEG AND SO IT IS AN ARGUMENT. The same event reaches one
/// destination silently and another reporting, so the mode cannot live on the
/// event. Statements: S126, S128.
pub trait NotificationDestination {
    fn deliver(&self, event: &Event, mode: ReportMode) -> Delivery;
}

/// The approval round trip: hand the harness payload to the phone and wait,
/// bounded, for the operator to answer it.
///
/// TWO STEPS AND NOT ONE, because the caller acts between them. A forward that
/// really BEGAN suppresses this process's own phone leg, since the card moshi
/// is raising is one the surface model cannot know about, and that suppression
/// happens before anybody waits for an answer. Collapsing the pair would make
/// the suppression unobservable and the ordering untestable.
///
/// THE PAYLOAD CROSSES AS BYTES, never as a parsed event: it belongs to the
/// harness and reaches the other side byte for byte whether or not pns could
/// parse it.
///
/// The answer is the exit code the harness contract defines, and `None` from
/// `forward` is a spawn that never began, which is not a denial.
///
/// Checked against `blocking_event` (`src/main.rs:2339-2346`), where the
/// filter chain spawns and the `is_some` branch sets `PNS_SKIP_PHONE`, and
/// `gate_mode` (`src/main.rs:245`), which spawns and waits with no arming
/// between. Statements: S074, S076.
pub trait ApprovalForwarder {
    fn forward(&self, subcommand: &str, payload_json: &str) -> Option<Forwarded>;
    fn answer(&self, forwarded: Forwarded) -> i32;
}

/// A forward that really began, and nothing more. It carries no detail because
/// no caller reads one: its whole meaning is that a child exists to wait on.
#[derive(Debug, PartialEq, Eq)]
pub struct Forwarded(pub u32);

/// The catch-up: whatever the journal is holding, delivered now.
///
/// WHETHER TO REPLAY IS THE CALLER'S QUESTION, not this port's. The use case
/// asks the domain first and only then calls this, so an observation or a
/// nudge reaches no adapter at all and an ordering test can say so. What the
/// replay is delivered THROUGH, and which of the operator's channels count as
/// somewhere they would see it, is bound into the adapter. Statements: S106.
pub trait MissedReplay {
    fn replay(&self);
}

/// The lamps, signalled for this event.
///
/// LAST ON THE EVENT PATH, after every channel the operator might be waiting
/// on. It is part of the plan rather than a second invocation, but it talks to
/// a bridge over the network under a deadline, and nothing an operator reads
/// should queue behind decoration. It still fires for a plan that reached no
/// channel at all: the lights are not a leg. Statements: S218, S230.
pub trait LampSignal {
    fn pulse(&self, behaviour: Behaviour, presence: Option<&Snapshot>);
}
