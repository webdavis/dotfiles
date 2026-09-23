use super::{RecapPolicy, ReplayMissedNotifications};
use crate::ports::delivery::{RecapPublisher, ReplayDelivery};
use crate::ports::records::{ActivityRing, Claim, OpenWaits, ReturnMoment};
use pns_domain::EventArgs;
use pns_domain::missed::Entry;
use pns_domain::routing::{Leg, ReportMode};
use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
use pns_domain::{Decision, GateInputs};
use std::cell::RefCell;

/// One log for every port, so the ORDER and the refusals are both assertable.
struct Recorder {
    steps: RefCell<Vec<String>>,
    claim: Option<Claim>,
    entries: Vec<Entry>,
    open: Vec<pns_domain::missed::OpenWait>,
    posted: bool,
    takes_card: bool,
    delivered: RefCell<Vec<String>>,
    handed: RefCell<Vec<(String, Vec<Leg>, crate::SubmissionIdentity)>>,
    claims: RefCell<Vec<(Option<u64>, bool)>>,
    publications: RefCell<Vec<(u64, u64)>>,
    delivery_legs: RefCell<Vec<Vec<Leg>>>,
    handoff: crate::ReplayHandoff,
    completed: std::cell::Cell<usize>,
    identities: RefCell<Vec<crate::SubmissionIdentity>>,
}

impl Recorder {
    fn new(claim: Option<Claim>) -> Self {
        Self {
            steps: RefCell::new(Vec::new()),
            claim,
            entries: Vec::new(),
            open: Vec::new(),
            posted: true,
            takes_card: true,
            delivered: RefCell::new(Vec::new()),
            handed: RefCell::new(Vec::new()),
            claims: RefCell::new(Vec::new()),
            publications: RefCell::new(Vec::new()),
            delivery_legs: RefCell::new(Vec::new()),
            handoff: crate::ReplayHandoff::Queued,
            completed: std::cell::Cell::new(0),
            identities: RefCell::new(Vec::new()),
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
    fn complete(&self) {
        self.completed.set(self.completed.get() + 1);
    }
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<Claim> {
        self.note(&format!("claim(journal={take_journal})"));
        self.claims.borrow_mut().push((now, take_journal));
        self.claim.as_ref().map(|held| Claim {
            since: held.since,
            waiting: held.waiting.clone(),
            replay: held.replay.clone(),
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
impl OpenWaits for Recorder {
    fn open_waits(&self, since: u64, until: u64) -> Vec<pns_domain::missed::OpenWait> {
        self.note(&format!("open_waits({since},{until})"));
        self.open.clone()
    }
}
impl RecapPublisher for Recorder {
    type Started = ();

    fn route(&self) -> Option<String> {
        Some("logbook".to_string())
    }

    fn publish(&self, since: u64, until: u64) -> Option<Self::Started> {
        self.note(&format!("publish({since},{until})"));
        self.publications.borrow_mut().push((since, until));
        self.posted.then_some(())
    }

    fn hand_card(&self, (): Self::Started, card: &crate::ReplayCard<'_>) -> bool {
        self.note("hand_card");
        self.handed.borrow_mut().push((
            card.detail.to_string(),
            card.legs.to_vec(),
            card.identity.clone(),
        ));
        self.takes_card
    }
}
impl ReplayDelivery for Recorder {
    fn deliver(
        &self,
        identity: &crate::SubmissionIdentity,
        event: &EventArgs,
        legs: &[Leg],
    ) -> crate::ReplayHandoff {
        self.identities.borrow_mut().push(identity.clone());
        self.note("deliver");
        self.delivered.borrow_mut().push(event.detail.clone());
        self.delivery_legs.borrow_mut().push(legs.to_vec());
        self.handoff
    }
}

fn ports(recorder: &Recorder) -> ReplayMissedNotifications<'_, Recorder> {
    ReplayMissedNotifications { ports: recorder }
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
        name: if decorative { "banner" } else { "hermes" },
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
            scope: pns_domain::DeliveryScope::Automatic,
        },
    }
}

fn policy() -> RecapPolicy {
    RecapPolicy {
        replay_card: true,
        post_window_recap: true,
        minimum_events: 2,
    }
}

fn claim_of(since: Option<u64>, waiting: Vec<Entry>) -> Claim {
    Claim {
        since,
        waiting,
        replay: Some(crate::ReplayBatch {
            identity: crate::SubmissionIdentity {
                producer: "pns-return".into(),
                request_id: "original-batch".into(),
            },
            until: Some(2_000),
            state: crate::ReplayState::Unsubmitted,
        }),
    }
}

#[test]
fn a_return_claims_the_moment_counts_the_window_publishes_then_hands_the_card_over() {
    // THE ORDERING THE CARD'S OWNERSHIP RESTS ON. The child is started first,
    // because the card's own sentence says whether a recap is coming, and the
    // composed card is handed to that child rather than delivered here, so the
    // process holding the rendered recap is the one that dispatches it.
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    let legs = vec![
        leg(true),
        Leg {
            mode: ReportMode::ReportOutcome,
            ..leg(false)
        },
    ];
    ports(&recorder).run(&returning(legs.clone()), policy(), true);
    assert_eq!(*recorder.claims.borrow(), [(Some(2_000), true)]);
    assert_eq!(*recorder.publications.borrow(), [(1_000, 2_000)]);
    assert!(
        recorder.delivery_legs.borrow().is_empty(),
        "a card the child took was delivered here as well"
    );
    let handed = recorder.handed.borrow()[0].clone();
    assert_eq!(handed.1, legs, "the child was handed the plan's own legs");
    assert_eq!(
        handed.2,
        claim_of(Some(1_000), Vec::new())
            .replay
            .expect("a batch")
            .identity,
        "the child submits under another identity than this batch's"
    );
    assert_eq!(
        recorder.steps(),
        [
            "claim(journal=true)",
            "entries(1000,2000)",
            "publish(1000,2000)",
            "open_waits(1000,2000)",
            "hand_card",
        ]
    );
    assert_eq!(
        recorder.completed.get(),
        1,
        "a batch whose card has an owner was left claimed"
    );
}

#[test]
fn a_child_that_will_not_take_the_card_leaves_it_with_this_process() {
    // THE HAND-OFF IS THE ONLY THING THAT MOVED. A child that died between
    // the spawn and the hand-off must not cost the operator the card, so the
    // card falls back to the delivery this process always did.
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    recorder.takes_card = false;
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert_eq!(recorder.handed.borrow().len(), 1, "{:?}", recorder.steps());
    assert_eq!(
        *recorder.delivered.borrow(),
        ["2 events. recap in #logbook"]
    );
    assert_eq!(
        recorder.steps(),
        [
            "claim(journal=true)",
            "entries(1000,2000)",
            "publish(1000,2000)",
            "open_waits(1000,2000)",
            "hand_card",
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
        post_window_recap: false,
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
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    // A loud window would publish if the post_window_recap switch were ignored.
    recorder.entries = vec![entry(1_100), entry(1_200)];
    let quiet = RecapPolicy {
        post_window_recap: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), quiet, true);
    assert!(recorder.steps().contains(&"deliver".to_string()));
    assert!(recorder.publications.borrow().is_empty());
    assert_eq!(
        *recorder.delivered.borrow(),
        ["1 missed notification. claude · stop: did a thing"]
    );
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
        post_window_recap: false,
        ..policy()
    };
    ports(&recorder).run(&returning(vec![leg(true)]), quiet, true);
    let card = recorder.delivered.borrow()[0].clone();
    assert_eq!(card, "1 missed notification. claude · stop: did a thing");
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
    assert_eq!(
        posted.handed.borrow()[0].0,
        "2 events. recap in #logbook",
        "the card the child took claims a recap nobody is writing"
    );
    assert!(
        failed.handed.borrow().is_empty(),
        "a card was handed to a child that never started"
    );
    assert_eq!(*failed.delivered.borrow(), ["2 events"]);
}

mod handoff;
mod open_waits;
