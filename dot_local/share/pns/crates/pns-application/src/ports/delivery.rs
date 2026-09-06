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

/// The approval round trip: hand the prompt to the phone and wait, bounded,
/// for the operator to answer it.
///
/// `None` IS THE DEADLINE EXPIRING, and it is not a denial. A phone that never
/// answers must release the prompt rather than hold it, so the caller
/// distinguishes "the operator said no" from "nobody said anything" and only
/// the first is a decision. The deadline itself belongs to the adapter, which
/// is what owns the child process and the socket.
pub trait ApprovalForwarder {
    fn ask(&self, event: &Event) -> Option<bool>;
}

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
