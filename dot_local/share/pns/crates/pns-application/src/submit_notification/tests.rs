use super::{Attempt, Submission, SubmitNotification};
use crate::ports::delivery::{LampSignal, MissedReplay};
use crate::ports::records::{
    ActivityRing, BlockedMarker, Claim, DecisionRing, Journal, LampRecords, LightsTick, LoopLease,
    ReturnMoment,
};
use pns_domain::decision::{Decision, GateInputs, Overrides};
use pns_domain::decision_record::Record;
use pns_domain::lamps::config::Behaviour;
use pns_domain::notification::EventArgs;
use pns_domain::presence::narrowing::Snapshot;
use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
use std::cell::RefCell;

/// EVERY PORT RECORDS INTO ONE LOG, which is what makes the ORDER assertable.
/// Ten separate spies could each prove they were called and none of them could
/// prove what came before it.
#[derive(Default)]
struct Recorder {
    steps: RefCell<Vec<String>>,
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
    fn read(&self) -> Option<String> {
        None
    }
}
impl Journal for Recorder {
    fn journal(&self, _event: &EventArgs, _now: Option<u64>) {
        self.note("journal");
    }
    fn read(&self) -> Option<String> {
        None
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
    fn replay(&self) {
        self.note("replay");
    }
}
impl ReturnMoment for Recorder {
    fn claim(&self, _now: Option<u64>, take_journal: bool) -> Option<Claim> {
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

fn ports(recorder: &Recorder) -> SubmitNotification<'_> {
    SubmitNotification {
        decisions: recorder,
        journal: recorder,
        blocked: recorder,
        lamp_records: recorder,
        lease: recorder,
        activity: recorder,
        replay: recorder,
        moment: recorder,
        lamps: recorder,
        tick: recorder,
    }
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

// --- the order itself ---------------------------------------------------

#[test]
fn the_records_are_written_in_the_order_the_event_path_states() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    assert_eq!(
        run(submission(&event, &decision, &overrides)),
        [
            "decision",
            "marker(live)",
            "news(Done)",
            "lease",
            "activity",
            "replay",
            "edge",
            "pulse(Done)",
            "clear",
            "tick",
        ]
    );
}

#[test]
fn the_decision_line_is_written_before_anything_else() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    assert_eq!(steps.first().map(String::as_str), Some("decision"));
}

#[test]
fn the_journal_is_written_before_the_activity_ring() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    let journal = steps.iter().position(|step| step == "journal").unwrap();
    let activity = steps.iter().position(|step| step == "activity").unwrap();
    assert!(journal < activity, "{steps:?}");
}

#[test]
fn the_catch_up_runs_after_both_records_and_before_the_pulse() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    let activity = steps.iter().position(|step| step == "activity").unwrap();
    let replay = steps.iter().position(|step| step == "replay").unwrap();
    let pulse = steps
        .iter()
        .position(|step| step.starts_with("pulse"))
        .unwrap();
    assert!(activity < replay && replay < pulse, "{steps:?}");
}

#[test]
fn the_pulse_goes_after_every_record_the_operator_might_be_waiting_on() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    let pulse = steps
        .iter()
        .position(|step| step.starts_with("pulse"))
        .unwrap();
    for earlier in ["decision", "activity", "replay"] {
        let at = steps.iter().position(|step| step == earlier).unwrap();
        assert!(at < pulse, "{earlier} ran after the pulse: {steps:?}");
    }
}

#[test]
fn the_lights_tick_is_registered_last() {
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let steps = run(submission(&event, &decision, &overrides));
    assert_eq!(steps.last().map(String::as_str), Some("tick"));
}

// --- what each attempt is allowed to write ------------------------------

#[test]
fn a_nudge_writes_its_decision_line_and_stops() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let taken = Submission {
        attempt: Attempt::Nudge,
        ..submission(&event, &decision, &overrides)
    };
    assert_eq!(run(taken), ["decision(nag)"]);
}

#[test]
fn an_observation_writes_its_decision_line_and_stops() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    let taken = Submission {
        attempt: Attempt::Observation,
        ..submission(&event, &decision, &overrides)
    };
    assert_eq!(run(taken), ["decision"]);
}

#[test]
fn only_a_nudge_marks_its_decision_line_as_one() {
    let (event, decision, overrides) = (event(), missed_decision(), Overrides::default());
    for (attempt, expected) in [
        (Attempt::First, "decision"),
        (Attempt::Observation, "decision"),
        (Attempt::Nudge, "decision(nag)"),
    ] {
        let taken = Submission {
            attempt,
            ..submission(&event, &decision, &overrides)
        };
        assert_eq!(run(taken).first().map(String::as_str), Some(expected));
    }
}

// --- the gates on individual steps --------------------------------------

#[test]
fn an_event_nobody_missed_reaches_no_journal() {
    let (event, overrides) = (event(), Overrides::default());
    let mut decision = missed_decision();
    decision.plan.banner = true;
    let steps = run(submission(&event, &decision, &overrides));
    assert!(!steps.contains(&"journal".to_string()), "{steps:?}");
    assert!(steps.contains(&"activity".to_string()), "{steps:?}");
}

#[test]
fn a_machine_with_no_live_lamps_writes_no_marker_start_no_clear_and_no_tick() {
    // PRESENT AT THE DESK ON PURPOSE. With an absent operator the clear is
    // already skipped for its own reason, so an unguarded clear would still
    // look correct here and the missing guard would go unnoticed.
    let (event, decision, overrides) = (event(), delivered_decision(), Overrides::default());
    let taken = Submission {
        lamps_live: false,
        ..submission(&event, &decision, &overrides)
    };
    let steps = run(taken);
    assert!(steps.contains(&"marker".to_string()), "{steps:?}");
    assert!(!steps.contains(&"clear".to_string()), "{steps:?}");
    assert!(!steps.contains(&"tick".to_string()), "{steps:?}");
}

#[test]
fn the_return_edge_is_claimed_without_taking_the_journal_with_it() {
    let (event, overrides) = (event(), Overrides::default());
    let mut decision = missed_decision();
    decision.inputs.surface = Surface::Desk;
    let steps = run(submission(&event, &decision, &overrides));
    assert!(steps.contains(&"edge".to_string()), "{steps:?}");
    assert!(!steps.contains(&"edge(journal)".to_string()), "{steps:?}");
}

#[test]
fn a_blocked_event_pulses_even_where_the_plan_did_not_ask_for_one() {
    let (overrides, decision) = (Overrides::default(), missed_decision());
    let event = EventArgs {
        state: "blocked".to_string(),
        ..event()
    };
    let steps = run(submission(&event, &decision, &overrides));
    assert!(steps.contains(&"pulse(Blocked)".to_string()), "{steps:?}");
}

#[test]
fn a_silenced_blocked_event_pulses_nothing() {
    let decision = missed_decision();
    let event = EventArgs {
        state: "blocked".to_string(),
        ..event()
    };
    let overrides = Overrides {
        muted: true,
        ..Overrides::default()
    };
    let steps = run(submission(&event, &decision, &overrides));
    assert!(
        !steps.iter().any(|step| step.starts_with("pulse")),
        "{steps:?}"
    );
}
