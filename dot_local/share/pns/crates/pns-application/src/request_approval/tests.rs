use super::RequestApproval;

#[test]
fn a_gate_forwards_the_exact_prompt_and_only_waits_for_its_answer() {
    let recorder = Recorder::new(Some(Forwarded(19)), 23);
    let code = ports(&recorder).forward_only("pi-hook", "{\"ask\":\"$(untouched)\"}\n");
    assert_eq!(
        recorder.steps(),
        ["forward(pi-hook,{\"ask\":\"$(untouched)\"}\n)", "answer"]
    );
    assert_eq!(code, 23);
}

#[test]
fn a_gate_whose_forward_never_started_has_no_answer_or_other_effect() {
    let recorder = Recorder::new(None, 23);
    let code = ports(&recorder).forward_only("omp-hook", "{}\n");
    assert_eq!(recorder.steps(), ["forward(omp-hook,{}\n)"]);
    assert_eq!(code, 0);
}
use crate::ports::delivery::ApprovalForwarder;
use crate::ports::nag::NagSchedule;
use crate::ports::notification::{PhoneSuppression, RaiseNotification};
use pns_domain::EventArgs;
use std::cell::RefCell;

/// EVERY PORT WRITES INTO ONE LOG, which is what makes the ORDER assertable.
/// Four separate spies could each prove they were called and none of them
/// could prove what came before it.
#[derive(Debug, PartialEq, Eq)]
struct Forwarded(u32);

struct Recorder {
    steps: RefCell<Vec<String>>,
    /// Whether the forward begins. `None` is a spawn that never started.
    spawn: RefCell<Option<Forwarded>>,
    answer: i32,
}

impl Recorder {
    fn new(spawn: Option<Forwarded>, answer: i32) -> Self {
        Self {
            steps: RefCell::new(Vec::new()),
            spawn: RefCell::new(spawn),
            answer,
        }
    }
    fn note(&self, step: &str) {
        self.steps.borrow_mut().push(step.to_string());
    }
    fn steps(&self) -> Vec<String> {
        self.steps.borrow().clone()
    }
}

impl ApprovalForwarder for Recorder {
    type Forwarded = Forwarded;
    fn forward(&self, subcommand: &str, payload_json: &str) -> Option<Forwarded> {
        self.note(&format!("forward({subcommand},{payload_json})"));
        self.spawn.borrow_mut().take()
    }
    fn answer(&self, _forwarded: Forwarded) -> i32 {
        self.note("answer");
        self.answer
    }
}
impl PhoneSuppression for Recorder {
    fn suppress(&self) {
        self.note("suppress");
    }
}
impl NagSchedule for Recorder {
    fn arm(&self, session_id: &str, _event: &EventArgs) {
        self.note(&format!("arm({session_id})"));
    }
}
impl RaiseNotification for Recorder {
    fn raise(&self, _event: &EventArgs) {
        self.note("notify");
    }
}

fn ports(recorder: &Recorder) -> RequestApproval<'_, Recorder> {
    RequestApproval { ports: recorder }
}

fn event() -> EventArgs {
    EventArgs {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        ..EventArgs::default()
    }
}

#[test]
fn a_forwarded_prompt_runs_forward_suppress_arm_notify_wait_in_that_order() {
    let recorder = Recorder::new(Some(Forwarded(7)), 2);
    let code = ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
    assert_eq!(
        recorder.steps(),
        [
            "forward(claude-hook,{})",
            "suppress",
            "arm(session-1)",
            "notify",
            "answer",
        ]
    );
    assert_eq!(code, 2);
}

#[test]
fn the_forward_starts_before_anything_else() {
    let recorder = Recorder::new(Some(Forwarded(7)), 0);
    ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
    let steps = recorder.steps();
    assert!(steps[0].starts_with("forward("), "{steps:?}");
}

#[test]
fn the_phone_leg_is_suppressed_only_where_the_spawn_really_began() {
    let began = Recorder::new(Some(Forwarded(7)), 0);
    ports(&began).run(&event(), "session-1", Some("claude-hook"), "{}");
    assert!(began.steps().contains(&"suppress".to_string()));

    // A SPAWN THAT NEVER STARTED SUPPRESSES NOTHING, or a machine with no
    // moshi would lose its phone card outright.
    let never = Recorder::new(None, 0);
    ports(&never).run(&event(), "session-1", Some("claude-hook"), "{}");
    assert!(!never.steps().contains(&"suppress".to_string()));
}

#[test]
fn an_agent_that_forwards_nothing_still_notifies_and_answers_zero() {
    let recorder = Recorder::new(Some(Forwarded(7)), 9);
    let code = ports(&recorder).run(&event(), "session-1", None, "{}");
    assert_eq!(recorder.steps(), ["arm(session-1)", "notify"]);
    assert_eq!(code, 0);
}

#[test]
fn the_nag_is_armed_before_the_notification() {
    let recorder = Recorder::new(Some(Forwarded(7)), 0);
    ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
    let steps = recorder.steps();
    let arm = steps
        .iter()
        .position(|step| step.starts_with("arm("))
        .unwrap();
    let notify = steps.iter().position(|step| step == "notify").unwrap();
    assert!(arm < notify, "{steps:?}");
}

#[test]
fn the_wait_comes_after_every_other_step() {
    let recorder = Recorder::new(Some(Forwarded(7)), 0);
    ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
    let steps = recorder.steps();
    assert_eq!(steps.last().map(String::as_str), Some("answer"));
}

#[test]
fn a_spawn_that_never_began_is_never_waited_on_and_answers_zero() {
    let recorder = Recorder::new(None, 9);
    let code = ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
    assert!(!recorder.steps().contains(&"answer".to_string()));
    assert_eq!(code, 0);
}

#[test]
fn the_operators_answer_is_returned_rather_than_reinterpreted() {
    for expected in [0, 1, 2] {
        let recorder = Recorder::new(Some(Forwarded(7)), expected);
        let code = ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
        assert_eq!(code, expected);
    }
}

#[test]
fn the_payload_crosses_to_the_forwarder_byte_for_byte() {
    let recorder = Recorder::new(Some(Forwarded(7)), 0);
    ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{\"raw\":1}");
    assert!(
        recorder.steps()[0].contains("{\"raw\":1}"),
        "{:?}",
        recorder.steps()
    );
}

#[test]
fn the_notification_fires_even_where_nothing_was_forwarded() {
    let recorder = Recorder::new(None, 0);
    ports(&recorder).run(&event(), "session-1", Some("claude-hook"), "{}");
    assert!(recorder.steps().contains(&"notify".to_string()));
}
