use super::{Outcome, RunNag};
use crate::{Claimed, NagRecords, RaiseNotification};
use pns_domain::EventArgs;
use pns_domain::nag::Record;
use std::cell::RefCell;

struct Recorder {
    steps: RefCell<Vec<String>>,
    fire: bool,
    claimed: Vec<Claimed>,
    carded: RefCell<Vec<EventArgs>>,
}

impl Recorder {
    fn new(fire: bool, claimed: Vec<Claimed>) -> Self {
        Self {
            steps: RefCell::new(Vec::new()),
            fire,
            claimed,
            carded: RefCell::new(Vec::new()),
        }
    }
    fn note(&self, step: &str) {
        self.steps.borrow_mut().push(step.to_string());
    }
    fn steps(&self) -> Vec<String> {
        self.steps.borrow().clone()
    }
}

impl NagRecords for Recorder {
    fn claim_fire(&self, _now: u64) -> bool {
        self.note("claim_fire");
        self.fire
    }
    fn claim_due(&self, _now: u64) -> Vec<Claimed> {
        self.note("claim_due");
        self.claimed.clone()
    }
    fn drop_claim(&self, session_id: &str, _reason: Option<pns_domain::nag::Dropped>) {
        self.note(&format!("drop({session_id})"));
    }
    fn mark_answered(&self, session_id: &str) -> Result<(), String> {
        self.note(&format!("mark({session_id})"));
        Ok(())
    }
    fn clear_answered(&self, _session_id: &str) -> Result<(), String> {
        unreachable!("run does not arm")
    }
    fn publish(&self, _session_id: &str, _record: &Record) -> Result<(), String> {
        unreachable!("run does not arm")
    }
    fn drop_record(&self, _session_id: &str) -> Result<(), String> {
        unreachable!("run releases claims, not pending names")
    }
    fn clear_pending(&self) -> usize {
        unreachable!("disabled cleanup is a separate path")
    }
    fn release_fire(&self) {
        self.note("release_fire");
    }
}
impl RaiseNotification for Recorder {
    fn raise(&self, event: &EventArgs) {
        self.note("card");
        self.carded.borrow_mut().push(EventArgs {
            agent: event.agent.clone(),
            detail: event.detail.clone(),
            pane: event.pane.clone(),
            ..EventArgs::default()
        });
    }
}

fn record(armed: u64) -> Record {
    Record {
        agent: "claude".to_string(),
        project: "dotfiles".to_string(),
        branch: "main".to_string(),
        detail: "may I edit this file".to_string(),
        pane: "%3".to_string(),
        armed,
    }
}

fn claimed(session: &str, record: Option<Record>, answered: bool) -> Claimed {
    Claimed {
        session_id: session.to_string(),
        record,
        answered,
    }
}

const NOW: u64 = 10_000;
const AFTER: u64 = 60;

#[test]
fn a_run_that_cannot_take_the_fire_lock_does_nothing_at_all() {
    let recorder = Recorder::new(false, vec![claimed("s1", Some(record(NOW - 90)), false)]);
    assert_eq!(
        RunNag {
            records: &recorder,
            notifier: &recorder
        }
        .run(NOW, AFTER, |_| panic!("unexpected warning")),
        Outcome::Busy
    );
    assert_eq!(recorder.steps(), ["claim_fire"]);
}

#[test]
fn a_waiting_approval_is_marked_then_carded_then_dropped_and_the_lock_returned() {
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW - 90)), false)]);
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nudged(1));
    assert_eq!(
        recorder.steps(),
        [
            "claim_fire",
            "claim_due",
            "mark(s1)",
            "card",
            "drop(s1)",
            "release_fire"
        ]
    );
}

#[test]
fn nothing_waiting_gives_the_lock_back_and_raises_no_card() {
    let recorder = Recorder::new(true, Vec::new());
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nothing);
    assert_eq!(
        recorder.steps(),
        ["claim_fire", "claim_due", "release_fire"]
    );
    assert!(recorder.carded.borrow().is_empty());
}

#[test]
fn a_record_already_answered_is_dropped_rather_than_nudged_about() {
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW - 90)), true)]);
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nothing);
    assert!(
        recorder.steps().contains(&"drop(s1)".to_string()),
        "{:?}",
        recorder.steps()
    );
    assert!(!recorder.steps().contains(&"card".to_string()));
}

#[test]
fn a_record_nothing_could_parse_is_dropped_rather_than_counted() {
    let recorder = Recorder::new(true, vec![claimed("s1", None, false)]);
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nothing);
    assert!(recorder.steps().contains(&"drop(s1)".to_string()));
}

#[test]
fn a_wait_nobody_answered_for_twice_the_bound_is_dropped_as_abandoned() {
    // The backstop: a session that went away leaves a record nothing will
    // ever answer, and nudging about it forever is worse than forgetting it.
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW - 600)), false)]);
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nothing);
    assert!(recorder.steps().contains(&"drop(s1)".to_string()));
    assert!(!recorder.steps().contains(&"card".to_string()));
}

#[test]
fn the_card_is_about_the_oldest_wait_and_counts_them_all() {
    let recorder = Recorder::new(
        true,
        vec![
            claimed("young", Some(record(NOW - 30)), false),
            claimed("old", Some(record(NOW - 100)), false),
        ],
    );
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nudged(2));
    let carded = recorder.carded.borrow();
    // The count is every wait; the age is the OLDEST one's, which is what
    // sends the operator to the prompt that has been sitting longest.
    assert!(carded[0].detail.contains('2'), "{}", carded[0].detail);
    assert!(carded[0].detail.contains("1m"), "{}", carded[0].detail);
}

#[test]
fn every_waiting_session_is_marked_and_not_only_the_one_carded() {
    let recorder = Recorder::new(
        true,
        vec![
            claimed("s1", Some(record(NOW - 100)), false),
            claimed("s2", Some(record(NOW - 80)), false),
        ],
    );
    RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    let steps = recorder.steps();
    assert!(steps.contains(&"mark(s1)".to_string()), "{steps:?}");
    assert!(steps.contains(&"mark(s2)".to_string()), "{steps:?}");
}

#[test]
fn every_session_is_marked_before_the_card_is_raised() {
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW - 100)), false)]);
    RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    let steps = recorder.steps();
    let mark = steps.iter().position(|step| step == "mark(s1)").unwrap();
    let card = steps.iter().position(|step| step == "card").unwrap();
    assert!(mark < card, "{steps:?}");
}

#[test]
fn the_fire_lock_is_returned_on_the_carding_path_too() {
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW - 100)), false)]);
    RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(
        recorder.steps().last().map(String::as_str),
        Some("release_fire")
    );
}

#[test]
fn the_card_carries_the_oldest_records_own_pane_and_agent() {
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW - 100)), false)]);
    RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    let carded = recorder.carded.borrow();
    assert_eq!(carded[0].agent, "claude");
    assert_eq!(carded[0].pane, "%3");
}

#[test]
fn a_record_armed_after_the_clock_is_dropped_rather_than_aged_backwards() {
    // A future arming would underflow into an enormous wait, or saturate into
    // a fresh one; neither is a wait the operator has been sitting through.
    let recorder = Recorder::new(true, vec![claimed("s1", Some(record(NOW + 500)), false)]);
    let outcome = RunNag {
        records: &recorder,
        notifier: &recorder,
    }
    .run(NOW, AFTER, |_| panic!("unexpected warning"));
    assert_eq!(outcome, Outcome::Nothing);
}
