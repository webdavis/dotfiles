use super::*;
use pns_application::{ActivityRing, Journal, RecapPublisher, ReplayDelivery};
use pns_domain::routing::{Leg, ReportMode};
use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
use pns_domain::{Decision, EventArgs, GateInputs, missed::Entry};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Attempt {
    Completed,
    Failed,
    Interrupted,
}

pub(super) struct Replay {
    pub state: PathBuf,
    pub moment: FileReturnMoment,
    pub outcome: Cell<Option<Attempt>>,
    attempt: Attempt,
}

impl Replay {
    pub fn new(name: &str, attempt: Attempt) -> Self {
        let state = crate::state_fixtures::scratch(name);
        Journal::journal(
            &crate::FileRecords::new(state.clone()),
            &EventArgs {
                detail: "still owed".into(),
                ..EventArgs::default()
            },
            Some(1_000),
        );
        Self {
            moment: FileReturnMoment::new(state.clone()),
            state,
            outcome: Cell::new(None),
            attempt,
        }
    }

    pub fn run(&self) {
        pns_application::ReplayMissedNotifications { ports: self }.run(
            &returning(),
            pns_application::RecapPolicy {
                replay_card: true,
                digest: false,
                min_events: 2,
            },
            false,
        );
    }
}

impl ReturnMoment for Replay {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<Claim> {
        self.moment.claim(now, take_journal)
    }
    fn complete(&self) {
        self.moment.complete();
    }
}

impl ActivityRing for Replay {
    fn record(&self, _: &EventArgs, _: Option<u64>) {
        panic!("replay must not record activity");
    }
    fn entries_between(&self, _: u64, _: u64) -> Vec<Entry> {
        Vec::new()
    }
}

impl RecapPublisher for Replay {
    fn publish(&self, _: u64, _: u64) -> bool {
        panic!("digest is disabled");
    }
}

impl ReplayDelivery for Replay {
    fn deliver(&self, event: &EventArgs, _: &[Leg]) {
        assert_eq!(
            holds(&self.state).len(),
            1,
            "delivery must still own the on-disk batch"
        );
        assert!(event.detail.contains("still owed"));
        if self.attempt == Attempt::Interrupted {
            panic!("owned replay interrupted");
        }
        // The legacy port returns after either outcome. Both completed paths
        // consume the batch; retry policy is a separate delivery-ledger change.
        self.outcome.set(Some(self.attempt));
    }
}

pub(super) fn holds(state: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(state)
        .expect("the state directory")
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("missed-notifications.held.")
        })
        .map(|entry| entry.path())
        .collect()
}

fn returning() -> Decision {
    Decision {
        legs: vec![Leg {
            name: "macos-banner",
            mode: ReportMode::Silent,
            decorative: true,
        }],
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
