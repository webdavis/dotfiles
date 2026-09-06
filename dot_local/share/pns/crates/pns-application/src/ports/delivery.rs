//! Carrying a signal outward: the destinations, and the approval round trip.

use pns_domain::notification::Event;
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
