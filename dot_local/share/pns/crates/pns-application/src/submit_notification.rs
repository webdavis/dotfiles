//! One notification's RECORDS, in the order the event path writes them.
//!
//! THE ORDER IS THE BEHAVIOR, which is why it is a use case and not a list of
//! calls at the composition root. Each step here was placed against the ones
//! around it for a reason the tests beside this file state one at a time, and
//! a reordering that still compiles is a defect no type can catch.
//!
//! WHAT IS NOT HERE: deciding the plan, taking the presence snapshot and
//! dispatching the legs. Those read the config and the operator's secrets and
//! stay at the composition root for now; this owns everything from the
//! decision record onward, which is where the ordering contract lives.

use crate::ports::delivery::{LampSignal, MissedReplay};
use crate::ports::records::{
    ActivityRing, BlockedMarker, DecisionRing, Journal, LampRecords, LightsTick, LoopLease,
    ReturnMoment,
};
use pns_domain::decision::{Decision, Overrides};
use pns_domain::decision_record::Record;
use pns_domain::lamps::config::Behaviour;
use pns_domain::missed;
use pns_domain::notification::EventArgs;
use pns_domain::presence::narrowing::Snapshot;
use pns_domain::pulse;

/// Which delivery of one prompt this is.
///
/// ONLY THE FIRST WRITES THE TAIL. A nudge is a second card about an approval
/// already recorded, and an observation is a card nobody asked for; both write
/// the decision line, so the log says a card fired, and neither may journal a
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
/// ONE STRUCT AND NOT TWELVE ARGUMENTS, for `Snapshot`'s own reason: these are
/// one event's readings, and a caller free to take any of them again further
/// down is a caller free to take it at a different moment.
pub struct Submission<'a> {
    pub event: &'a EventArgs,
    pub decision: &'a Decision,
    pub overrides: &'a Overrides,
    pub legs: &'a [(pns_domain::routing::Leg, pns_domain::routing::Delivery)],
    pub attempt: Attempt,
    /// The harness payload's own identity, empty where the event carries none.
    pub session_id: &'a str,
    pub permission_mode: &'a str,
    pub agent_id: &'a str,
    pub tool_name: &'a str,
    /// Whether a lamp map AND a transport are both live. A marker written with
    /// no lamp to read it is a wait nothing will ever clear.
    pub lamps_live: bool,
    /// Whether the config declared any lamps at all, which is a weaker
    /// question than `lamps_live` and the one the behaviour is read against.
    pub lights_declared: bool,
    pub presence: Option<&'a Snapshot>,
}

/// The ports the tail writes through.
pub struct SubmitNotification<'a> {
    pub decisions: &'a dyn DecisionRing,
    pub journal: &'a dyn Journal,
    pub blocked: &'a dyn BlockedMarker,
    pub lamp_records: &'a dyn LampRecords,
    pub lease: &'a dyn LoopLease,
    pub activity: &'a dyn ActivityRing,
    pub replay: &'a dyn MissedReplay,
    pub moment: &'a dyn ReturnMoment,
    pub lamps: &'a dyn LampSignal,
    pub tick: &'a dyn LightsTick,
}

impl SubmitNotification<'_> {
    /// Write this event's records, in order.
    pub fn record(&self, submission: &Submission) {
        let decision = submission.decision;
        let overrides = submission.overrides;

        self.decisions.record(&Record {
            event: submission.event,
            decision,
            overrides,
            legs: submission.legs,
            nag: submission.attempt == Attempt::Nudge,
            permission_mode: submission.permission_mode,
            agent_id: submission.agent_id,
            tool_name: submission.tool_name,
        });

        // THE CONTIGUOUS TAIL BELOW BELONGS TO THE FIRST DELIVERY. A nudge or
        // an observation returns here, so it writes no journal entry, no
        // activity line, claims no return moment and never pulses.
        if submission.attempt != Attempt::First {
            return;
        }

        // ASKED HERE RATHER THAN INSIDE THE PORT, so a test can say that an
        // event nobody missed reaches no journal at all.
        if missed::was_missed(decision, overrides) {
            self.journal
                .journal(submission.event, decision.inputs.now_secs);
        }

        self.blocked.update(
            submission.session_id,
            &submission.event.state,
            submission.lamps_live,
            decision.inputs.now_secs,
        );
        self.lamp_records.news(
            pulse::state_behaviour(&submission.event.state, true),
            decision.inputs.now_secs,
        );
        self.lease
            .renew(&submission.event.pane, decision.inputs.now_secs);
        // UNCONDITIONALLY, which is the whole difference between it and the
        // journal above: the recap's window is every event, delivered or not.
        self.activity
            .record(submission.event, decision.inputs.now_secs);

        // THE CATCH-UP GOES AFTER BOTH RECORDS AND BEFORE THE EDGE: a slow
        // replay must not cost either record, and the edge below closes the
        // window this reads.
        if missed::should_replay(decision) {
            self.replay.replay();
        }
        if missed::is_present(decision) {
            self.moment.claim(decision.inputs.now_secs, false);
        }

        // THE PULSE GOES LAST, after every channel the operator might be
        // waiting on. It still fires for a plan that reached no channel at
        // all: the lights are not a leg.
        let behaviour = pulse::state_behaviour(&submission.event.state, submission.lights_declared);
        let blocked_lamp = behaviour == Behaviour::Blocked && !overrides.silenced();
        if decision.plan.pulse || blocked_lamp {
            self.lamps.pulse(behaviour, submission.presence);
        }
        if submission.lamps_live && missed::is_present(decision) {
            self.lamp_records.clear_held();
        }
        if submission.lamps_live {
            self.tick.register(decision, overrides);
        }
    }
}

#[cfg(test)]
mod tests;
