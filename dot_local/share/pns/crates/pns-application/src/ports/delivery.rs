//! Carrying a signal outward: the destinations, and the approval round trip.

use crate::{DeliveryRequest, DestinationId};
use pns_domain::EventArgs;
use pns_domain::lamps::config::Behaviour;
use pns_domain::registry::Routing;
use pns_domain::routing::{Delivery, Leg};
use pns_domain::{Decision, Snapshot};

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
/// Checked against `dispatch_legs` (`src/main.rs:3434`), which walks the legs
/// and pairs each with what its destination answered.
pub trait NotificationDestination: Send + Sync {
    fn id(&self) -> &DestinationId;
    fn capabilities(&self) -> Routing;
    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery;
}

/// Submit the original harness payload and complete that submission, bounded.
///
/// TWO STEPS AND NOT ONE, because the caller acts between them. A forward that
/// really BEGAN suppresses this process's own phone leg, since the card moshi
/// is raising is one the surface model cannot know about, and that suppression
/// happens before anybody waits for submission completion. Collapsing the pair
/// would make the suppression unobservable and the ordering untestable.
///
/// THE PAYLOAD CROSSES UNCHANGED, never as a parsed event: it belongs to the
/// harness and reaches the other side byte for byte whether or not pns could
/// parse it. The original subcommand crosses beside it.
///
/// The answer is the submission's arbitrary exit code, not human approval or
/// denial. `None` from `forward` is a spawn that never began. The adapter owns
/// the child, its deadline and cleanup; completion consumes that owned child.
///
/// Checked against `gate_mode` and `blocking_event` in `src/moshi_submission.rs`,
/// including the submission semantics documented by `moshi_decision` and
/// `answer_within`. Statements: S074, S076.
pub trait ApprovalForwarder {
    type Forwarded;

    fn forward(&self, subcommand: &str, payload_json: &str) -> Option<Self::Forwarded>;
    fn answer(&self, forwarded: Self::Forwarded) -> i32;
}

/// The catch-up: whatever the journal is holding, delivered now.
///
/// WHETHER TO REPLAY IS THE CALLER'S QUESTION, not this port's. The use case
/// asks the domain first and only then calls this, so an observation or a
/// nudge reaches no adapter at all and an ordering test can say so. What the
/// replay is delivered through is bound into the adapter. The current decision
/// supplies its delivery legs and clock. Statements: S106.
pub trait MissedReplay {
    fn replay(&self, decision: &Decision);
}

/// The lamps, signalled for this event.
///
/// LAST ON THE EVENT PATH, after every channel the operator might be waiting
/// on. It is part of the plan rather than a second invocation, but it talks to
/// a bridge over the network under a deadline, and nothing an operator reads
/// should queue behind decoration. It still fires for a plan that reached no
/// channel at all: the lights are not a leg. Statements: S218.
pub trait LampSignal {
    fn pulse(&self, behaviour: Behaviour, presence: Option<&Snapshot>);
}

/// Start the return recap in a separate process.
///
/// The answer reports whether that process started. It does not confirm
/// publication: the caller does not wait for the child to render or post.
///
/// THE WINDOW IS THE ONLY ARGUMENT. Which repositories are read, how the
/// digest is composed and where it is posted are the adapter's; the use case
/// decides only that this window is worth publishing.
///
/// Checked against `spawn_recap` (`src/main.rs:1421`), whose decision this
/// takes and whose spawn stays in the root: `pns-application` names no child
/// process. Statements: S164, S165.
pub trait RecapPublisher {
    fn publish(&self, since: u64, until: u64) -> bool;
}

/// One already-decided notification, delivered over already-decided legs.
///
/// THE LEG WALK AND THE RENDER ARE MECHANICS, not decisions, so they stay
/// behind this port. The use case hands over the event it synthesized and the
/// legs the plan already chose; which destination each leg names, and how the
/// event becomes a card, are the dispatcher's.
///
/// Queued proves durable ledger ownership, including an existing submission.
/// A failed or unpersisted handoff retains the journal. The ledger owns retry.
///
/// Checked against `replay_missed`'s closing call into `dispatch_legs`
/// (`src/main.rs`), which passes the synthesized event, the decision's legs
/// and `pane_dropped` false. Statements: S106.
pub trait ReplayDelivery {
    fn deliver(
        &self,
        identity: &crate::SubmissionIdentity,
        event: &EventArgs,
        legs: &[Leg],
    ) -> ReplayHandoff;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayHandoff {
    Queued,
    Retained,
}
