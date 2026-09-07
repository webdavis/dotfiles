use super::{Attempt, Submission, SubmitNotification};
use crate::ports::delivery::{LampSignal, MissedReplay};
use crate::ports::records::{
    ActivityRing, BlockedMarker, Claim, DecisionRing, Journal, LampRecords, LightsTick, LoopLease,
    ReturnMoment,
};
use pns_domain::EventArgs;
use pns_domain::Record;
use pns_domain::Snapshot;
use pns_domain::lamps::config::Behaviour;
use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
use pns_domain::{Decision, GateInputs, Overrides};
use std::cell::RefCell;

/// EVERY PORT RECORDS INTO ONE LOG, which is what makes the ORDER assertable.
/// Ten separate spies could each prove they were called and none of them could
/// prove what came before it.
#[derive(Default)]
struct Recorder {
    steps: RefCell<Vec<String>>,
    replays: RefCell<Vec<(Option<u64>, Vec<pns_domain::routing::Leg>)>>,
    claims: RefCell<Vec<(Option<u64>, bool)>>,
}

impl Recorder {
    fn note(&self, step: &str) {
        self.steps.borrow_mut().push(step.to_string());
    }
    fn steps(&self) -> Vec<String> {
        self.steps.borrow().clone()
    }
}

impl DecisionRing for Recorder {
    fn record(&self, record: &Record) {
        self.note(if record.nag {
            "decision(nag)"
        } else {
            "decision"
        });
    }
    fn read(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
}
impl Journal for Recorder {
    fn journal(&self, _event: &EventArgs, _now: Option<u64>) {
        self.note("journal");
    }
    fn read(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
}
impl ActivityRing for Recorder {
    fn record(&self, _event: &EventArgs, _now: Option<u64>) {
        self.note("activity");
    }
    fn entries_between(&self, _since: u64, _until: u64) -> Vec<pns_domain::missed::Entry> {
        Vec::new()
    }
}
impl BlockedMarker for Recorder {
    fn update(&self, _session: &str, _state: &str, lamps_live: bool, _now: Option<u64>) {
        self.note(if lamps_live { "marker(live)" } else { "marker" });
    }
}
impl LoopLease for Recorder {
    fn renew(&self, _pane: &str, _now: Option<u64>) {
        self.note("lease");
    }
}
impl LampRecords for Recorder {
    fn news(&self, behaviour: Behaviour, _now: Option<u64>) {
        self.note(&format!("news({behaviour:?})"));
    }
    fn clear_held(&self) {
        self.note("clear");
    }
}
impl MissedReplay for Recorder {
    fn replay(&self, decision: &Decision) {
        self.replays
            .borrow_mut()
            .push((decision.inputs.now_secs, decision.legs.clone()));
        self.note("replay");
    }
}
impl ReturnMoment for Recorder {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<Claim> {
        self.claims.borrow_mut().push((now, take_journal));
        self.note(if take_journal {
            "edge(journal)"
        } else {
            "edge"
        });
        None
    }
}
impl LampSignal for Recorder {
    fn pulse(&self, behaviour: Behaviour, _presence: Option<&Snapshot>) {
        self.note(&format!("pulse({behaviour:?})"));
    }
}
impl LightsTick for Recorder {
    fn register(&self, _decision: &Decision, _overrides: &Overrides) {
        self.note("tick");
    }
}

fn ports(recorder: &Recorder) -> SubmitNotification<'_, Recorder> {
    SubmitNotification { ports: recorder }
}

/// A decision that delivered nothing anybody would see, which is what makes an
/// event a MISS: no banner, no card, and a surface that is not the desk.
fn missed_decision() -> Decision {
    Decision {
        legs: Vec::new(),
        plan: DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        },
        pane_dropped: false,
        inputs: GateInputs {
            desk_input_age: None,
            phone_input_age: None,
            marker_age: None,
            screen_locked: None,
            desk_fresh_secs: None,
            surface: Surface::Away,
            session_visibility: Visibility::Unknown,
            visibility: Visibility::Unknown,
            pane_present: false,
            now_secs: Some(1_700_000_000),
            long_running: false,
            mobile_watch_card: false,
            local_only: false,
            remote_only: false,
        },
    }
}

/// A decision where every gate on the tail is OPEN: present at the desk, a
/// banner delivered, and a pulse in the plan. The whole-order test needs one,
/// because a fixture that skips a step cannot say where that step belongs.
fn delivered_decision() -> Decision {
    let mut decision = missed_decision();
    decision.inputs.surface = Surface::Desk;
    decision.plan.banner = true;
    decision.plan.pulse = true;
    decision
}

fn event() -> EventArgs {
    EventArgs {
        agent: "claude".to_string(),
        state: "stop".to_string(),
        pane: "%3".to_string(),
        ..EventArgs::default()
    }
}

fn submission<'a>(
    event: &'a EventArgs,
    decision: &'a Decision,
    overrides: &'a Overrides,
) -> Submission<'a> {
    Submission {
        event,
        decision,
        overrides,
        legs: &[],
        attempt: Attempt::First,
        session_id: "session",
        permission_mode: "",
        agent_id: "",
        tool_name: "",
        lamps_live: true,
        lights_declared: true,
        presence: None,
    }
}

fn run(taken: Submission) -> Vec<String> {
    let recorder = Recorder::default();
    ports(&recorder).record(&taken);
    recorder.steps()
}

mod arguments;
mod attempts;
mod gates;
mod order;
