//! One notification's RECORDS, in the order the event path writes them.
//!
//! THE ORDER IS THE BEHAVIOR, which is why it is a use case and not a list of
//! calls at the composition root. Each step here was placed against the ones
//! around it for a reason the tests beside this file state one at a time, and
//! a reordering that still compiles is a defect no type can catch.
//!
//! Dispatch begins the durable decision and records each actual outcome before
//! this tail runs. The tail journals unperceived events under that original
//! identity, then updates activity and lamps in the established order.

use crate::SubmissionIdentity;
use crate::ports::delivery::{LampSignal, MissedReplay};
use crate::ports::records::{
    ActivityRing, BlockedMarker, Journal, LampRecords, LightsTick, LoopLease, ReturnMoment,
};
use pns_domain::EventArgs;
use pns_domain::Snapshot;
use pns_domain::lamps::config::Behaviour;
use pns_domain::missed;
use pns_domain::pulse;
use pns_domain::{Decision, Overrides};

/// Which delivery of one prompt this is.
///
/// ONLY THE FIRST WRITES THE TAIL. A nudge is a second card about an approval
/// already recorded, and an observation is a card nobody asked for. Their
/// decisions belong to dispatch too; neither may journal a
/// miss, count as activity, claim the return moment or pulse. Each of those is
/// a defect avoided rather than tidiness: the recap would count one prompt
/// twice, and the operator's return would close on a window one event wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    First,
    Nudge,
    Observation,
}

/// Everything the tail is a function of, taken at ONE moment.
///
/// One value groups this event's inputs, for `Snapshot`'s own reason: these are
/// one event's readings, and a caller free to take any of them again further
/// down is a caller free to take it at a different moment.
pub struct Submission<'a> {
    pub identity: Option<&'a SubmissionIdentity>,
    pub event: &'a EventArgs,
    pub decision: &'a Decision,
    pub overrides: &'a Overrides,
    pub legs: &'a [(pns_domain::routing::Leg, pns_domain::routing::Delivery)],
    pub attempt: Attempt,
    /// The harness payload's own identity, empty where the event carries none.
    pub session_id: &'a str,
    /// Whether a lamp map AND a transport are both live. A marker written with
    /// no lamp to read it is a wait nothing will ever clear.
    pub lamps_live: bool,
    /// Whether the config declared any lamps at all, which is a weaker
    /// question than `lamps_live` and the one the behaviour is read against.
    pub lights_declared: bool,
    pub presence: Option<&'a Snapshot>,
}

/// The ports the tail writes through.
pub struct SubmitNotification<'a, P> {
    pub ports: &'a P,
}

impl<P> SubmitNotification<'_, P>
where
    P: Journal
        + BlockedMarker
        + LampRecords
        + LoopLease
        + ActivityRing
        + MissedReplay
        + ReturnMoment
        + LampSignal
        + LightsTick,
{
    /// Write this event's records, in order.
    pub fn record(&self, submission: &Submission) {
        let decision = submission.decision;
        let overrides = submission.overrides;

        if submission.attempt != Attempt::First {
            return;
        }

        // ASKED HERE RATHER THAN INSIDE THE PORT, so a test can say that an
        // event nobody missed reaches no journal at all.
        let actual_miss = missed::was_missed(decision, overrides, submission.legs);
        if actual_miss {
            Journal::journal(
                self.ports,
                submission.event,
                decision.inputs.now_secs,
                submission.identity,
            );
        }

        BlockedMarker::update(
            self.ports,
            submission.session_id,
            &submission.event.state,
            submission.lamps_live,
            decision.inputs.now_secs,
        );
        LampRecords::news(
            self.ports,
            pulse::state_behaviour(&submission.event.state, true),
            decision.inputs.now_secs,
        );
        LoopLease::renew(self.ports, &submission.event.pane, decision.inputs.now_secs);
        // UNCONDITIONALLY, which is the whole difference between it and the
        // journal above: the recap's window is every event, delivered or not.
        ActivityRing::record(self.ports, submission.event, decision.inputs.now_secs);

        // THE CATCH-UP GOES AFTER BOTH RECORDS AND BEFORE THE EDGE: a slow
        // replay must not cost either record, and the edge below closes the
        // window this reads.
        // A class exception belongs to the original request. The return
        // summary remains unmarked and respects the same silence inputs.
        if !overrides.silenced() && missed::should_replay(decision) {
            MissedReplay::replay(self.ports, decision);
        }
        if missed::is_present(decision) {
            ReturnMoment::claim(self.ports, decision.inputs.now_secs, false);
        }

        // THE PULSE GOES LAST, after every channel the operator might be
        // waiting on. It still fires for a plan that reached no channel at
        // all: the lights are not a leg.
        let behaviour = pulse::state_behaviour(&submission.event.state, submission.lights_declared);
        let blocked_lamp = behaviour == Behaviour::Blocked && !overrides.silenced();
        if decision.plan.pulse || blocked_lamp {
            LampSignal::pulse(self.ports, behaviour, submission.presence);
        }
        if submission.lamps_live && missed::is_present(decision) {
            LampRecords::clear_held(self.ports);
        }
        if submission.lamps_live {
            LightsTick::register(self.ports, decision, actual_miss);
        }
    }
}

#[cfg(test)]
mod tests;
