use super::{RecapPolicy, ReplayMissedNotifications};
use crate::ports::delivery::{RecapPublisher, ReplayDelivery};
use crate::ports::records::{ActivityRing, Claim, ReturnMoment};
use pns_domain::decision::{Decision, GateInputs};
use pns_domain::missed::Entry;
use pns_domain::notification::EventArgs;
use pns_domain::routing::{Leg, ReportMode};
use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
use std::cell::RefCell;

/// One log for every port, so the ORDER and the refusals are both assertable.
struct Recorder {
    steps: RefCell<Vec<String>>,
    claim: Option<Claim>,
    entries: Vec<Entry>,
    posted: bool,
    delivered: RefCell<Vec<String>>,
}

impl Recorder {
    fn new(claim: Option<Claim>) -> Self {
        Self {
            steps: RefCell::new(Vec::new()),
            claim,
            entries: Vec::new(),
            posted: true,
            delivered: RefCell::new(Vec::new()),
        }
    }
    fn note(&self, step: &str) {
        self.steps.borrow_mut().push(step.to_string());
    }
    fn steps(&self) -> Vec<String> {
        self.steps.borrow().clone()
    }
}

impl ReturnMoment for Recorder {
    fn claim(&self, _now: Option<u64>, take_journal: bool) -> Option<Claim> {
        self.note(&format!("claim(journal={take_journal})"));
        self.claim.as_ref().map(|held| Claim {
            since: held.since,
            waiting: held.waiting.clone(),
        })
    }
}
impl ActivityRing for Recorder {
    fn record(&self, _event: &EventArgs, _now: Option<u64>) {}
    fn entries_between(&self, since: u64, until: u64) -> Vec<Entry> {
        self.note(&format!("entries({since},{until})"));
        self.entries.clone()
    }
}
impl RecapPublisher for Recorder {
    fn publish(&self, since: u64, until: u64) -> bool {
        self.note(&format!("publish({since},{until})"));
        self.posted
    }
}
impl ReplayDelivery for Recorder {
    fn deliver(&self, event: &EventArgs, _legs: &[Leg]) {
        self.note("deliver");
        self.delivered.borrow_mut().push(event.detail.clone());
    }
}

fn ports(recorder: &Recorder) -> ReplayMissedNotifications<'_> {
    ReplayMissedNotifications {
        moment: recorder,
        activity: recorder,
        publisher: recorder,
        delivery: recorder,
    }
}

fn entry(at: u64) -> Entry {
    Entry {
        agent: "claude".to_string(),
        state: "stop".to_string(),
        project: String::new(),
        branch: String::new(),
        detail: "did a thing".to_string(),
        at: Some(at),
    }
}

fn leg(decorative: bool) -> Leg {
    Leg {
        name: if decorative { "macos-banner" } else { "hermes" },
        mode: ReportMode::Silent,
        decorative,
    }
}

/// A decision the catch-up accepts: present, delivered, with a decorative leg.
fn returning(legs: Vec<Leg>) -> Decision {
    Decision {
        legs,
        plan: DeliveryPlan {
            banner: true,
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
            surface: Surface::Desk,
            session_visibility: Visibility::Unknown,
            visibility: Visibility::Unknown,
            pane_present: false,
            now_secs: Some(2_000),
            long_running: false,
            mobile_watch_card: false,
            local_only: false,
            remote_only: false,
        },
    }
}

fn policy() -> RecapPolicy {
    RecapPolicy {
        replay_card: true,
        digest: true,
        min_events: 2,
    }
}

fn claim_of(since: Option<u64>, waiting: Vec<Entry>) -> Claim {
    Claim { since, waiting }
}

#[test]
fn a_return_claims_the_moment_counts_the_window_publishes_then_delivers() {
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert_eq!(
        recorder.steps(),
        [
            "claim(journal=true)",
            "entries(1000,2000)",
            "publish(1000,2000)",
            "deliver",
        ]
    );
}

#[test]
fn an_event_the_domain_says_is_no_replay_touches_nothing() {
    // Away is what the recap brackets; an away event is not a return.
    let recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    let mut decision = returning(vec![leg(true)]);
    decision.inputs.surface = Surface::Away;
    ports(&recorder).run(&decision, policy(), true);
    assert!(recorder.steps().is_empty(), "{:?}", recorder.steps());
}

#[test]
fn a_plan_with_no_decorative_leg_claims_nothing_and_delivers_nothing() {
    // A durable-only plan would post the catch-up into a log that already
    // holds all of it, and delete it, with nothing the operator ever sees.
    let recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    ports(&recorder).run(&returning(vec![leg(false)]), policy(), true);
    assert!(recorder.steps().is_empty(), "{:?}", recorder.steps());
}

#[test]
fn a_moment_somebody_else_holds_stops_the_catch_up_dead() {
    let recorder = Recorder::new(None);
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert_eq!(recorder.steps(), ["claim(journal=true)"]);
}

#[test]
fn the_journal_is_claimed_with_the_moment_only_where_a_card_may_be_raised() {
    let recorder = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    let silent = RecapPolicy {
        replay_card: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), silent, true);
    assert_eq!(recorder.steps()[0], "claim(journal=false)");
}

#[test]
fn a_digest_needs_a_durable_route_to_go_to() {
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), false);
    assert!(
        !recorder
            .steps()
            .iter()
            .any(|step| step.starts_with("publish")),
        "{:?}",
        recorder.steps()
    );
}

#[test]
fn a_window_thinner_than_the_operators_bar_publishes_no_digest() {
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    recorder.entries = vec![entry(1_100)];
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert!(
        !recorder
            .steps()
            .iter()
            .any(|step| step.starts_with("publish")),
        "{:?}",
        recorder.steps()
    );
}

#[test]
fn a_card_the_operator_turned_off_is_not_raised_though_the_digest_still_is() {
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    let silent = RecapPolicy {
        replay_card: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), silent, true);
    let steps = recorder.steps();
    assert!(
        steps.iter().any(|step| step.starts_with("publish")),
        "{steps:?}"
    );
    assert!(!steps.contains(&"deliver".to_string()), "{steps:?}");
}

#[test]
fn a_return_with_no_digest_and_nothing_waiting_says_nothing_at_all() {
    let recorder = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    let quiet = RecapPolicy {
        digest: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), quiet, true);
    assert!(
        !recorder.steps().contains(&"deliver".to_string()),
        "{:?}",
        recorder.steps()
    );
}

#[test]
fn entries_waiting_with_no_digest_are_summarized_rather_than_dropped() {
    let recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    let quiet = RecapPolicy {
        digest: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), quiet, true);
    assert!(recorder.steps().contains(&"deliver".to_string()));
    assert!(!recorder.delivered.borrow()[0].is_empty());
}

#[test]
fn a_marker_that_opened_no_window_counts_nothing_and_publishes_nothing() {
    // No marker is a first run, not a window from the epoch.
    let recorder = Recorder::new(Some(claim_of(None, vec![entry(1_500)])));
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    let steps = recorder.steps();
    assert!(
        !steps.iter().any(|step| step.starts_with("entries")),
        "{steps:?}"
    );
    assert!(
        !steps.iter().any(|step| step.starts_with("publish")),
        "{steps:?}"
    );
    assert!(steps.contains(&"deliver".to_string()), "{steps:?}");
}

#[test]
fn a_marker_newer_than_the_clock_opens_no_window_either() {
    let recorder = Recorder::new(Some(claim_of(Some(9_999), vec![entry(1_500)])));
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert!(
        !recorder
            .steps()
            .iter()
            .any(|step| step.starts_with("entries")),
        "{:?}",
        recorder.steps()
    );
}

#[test]
fn the_card_is_composed_from_the_journal_and_not_from_the_decision() {
    // WHAT THE OPERATOR MISSED IS WHAT THE JOURNAL HOLDS. The event that
    // triggered the return contributes the moment and the legs, never a word
    // of the sentence.
    let recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    let quiet = RecapPolicy {
        digest: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), quiet, true);
    let card = recorder.delivered.borrow()[0].clone();
    assert!(card.contains("claude"), "{card}");
}

#[test]
fn a_failed_publish_still_raises_a_card_and_the_card_says_which() {
    // THE CARD CLAIMS THE DIGEST IS FILED SOMEWHERE THE OPERATOR CAN READ IT.
    // A publish that failed must not be described that way, so the verdict is
    // carried into the sentence rather than dropped.
    let mut posted = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    posted.entries = vec![entry(1_100), entry(1_200)];
    ports(&posted).run(&returning(vec![leg(true)]), policy(), true);

    let mut failed = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    failed.entries = vec![entry(1_100), entry(1_200)];
    failed.posted = false;
    ports(&failed).run(&returning(vec![leg(true)]), policy(), true);

    assert!(failed.steps().contains(&"deliver".to_string()));
    assert_ne!(
        posted.delivered.borrow()[0],
        failed.delivered.borrow()[0],
        "the card reads the same whether or not the digest was published"
    );
}
