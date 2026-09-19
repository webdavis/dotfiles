use super::{EscalateStaleBlocks, StaleOutcome};
use crate::{RaiseNotification, StaleWaits};
use pns_domain::stale::{Blocked, Suppressed};
use pns_domain::surface::Surface;
use pns_domain::{EventArgs, SurfaceReading};
use std::cell::RefCell;

const WINDOW: u64 = 3_600;
const NOW: u64 = 10_000;

#[derive(Default)]
struct Recorder {
    rows: Vec<Blocked>,
    claimed: RefCell<Vec<String>>,
    pages: RefCell<Vec<EventArgs>>,
    refuse: &'static str,
}
impl StaleWaits for Recorder {
    fn waiting_since(&self, threshold: u64) -> Vec<Blocked> {
        self.rows
            .iter()
            .filter(|row| row.since <= threshold)
            .cloned()
            .collect()
    }
    fn claim(&self, session_id: &str, _now: u64) -> bool {
        self.claimed.borrow_mut().push(session_id.to_string());
        self.refuse != session_id
    }
}
impl RaiseNotification for Recorder {
    fn raise(&self, event: &EventArgs) {
        // WHOLE, never field by field: a page whose kind the recorder dropped
        // is a page these cases could not tell from one bound for the routine
        // route.
        self.pages.borrow_mut().push(event.clone());
    }
}

fn row(session: &str, since: u64) -> Blocked {
    Blocked {
        session: session.to_string(),
        harness: "claude".to_string(),
        project: "dotfiles".to_string(),
        branch: "feat/x".to_string(),
        title: "a title".to_string(),
        since,
    }
}

/// At the desk, unlocked: the operator can act.
fn at_the_desk() -> SurfaceReading {
    SurfaceReading {
        surface: Surface::Desk,
        phone_input_fresh: false,
        desk_input_age: Some(30),
        phone_input_age: None,
        marker_age: None,
        screen_locked: Some(false),
        desk_fresh_secs: Some(120),
    }
}

fn away() -> SurfaceReading {
    SurfaceReading {
        surface: Surface::Away,
        desk_input_age: None,
        ..at_the_desk()
    }
}

fn fired(recorder: &Recorder, reading: &SurfaceReading) -> StaleOutcome {
    EscalateStaleBlocks {
        waits: recorder,
        notifier: recorder,
    }
    .run(NOW, WINDOW, "", reading)
}

#[test]
fn a_stale_block_is_claimed_before_it_is_paged_about() {
    let recorder = Recorder {
        rows: vec![row("s1", NOW - WINDOW)],
        ..Default::default()
    };
    assert_eq!(fired(&recorder, &at_the_desk()), StaleOutcome::Paged(1));
    assert_eq!(*recorder.claimed.borrow(), ["s1"]);
    let pages = recorder.pages.borrow();
    assert_eq!(pages.len(), 1);
    // THE PAGE NAMES A KIND, NEVER A ROUTE: the route it lands on is the one
    // `[routes] urgent` spells, resolved on the event path this fire raises
    // the page through.
    assert!(pages[0].channel.is_empty(), "{}", pages[0].channel);
    assert_eq!(pages[0].delivery_class, pns_domain::routes::HEALTH);
    assert_eq!(pages[0].detail, "blocked 60 minutes, no answer");
    assert_eq!(pages[0].session, "s1");
}

#[test]
fn an_away_operator_is_not_paged_and_no_row_is_stamped() {
    // Nothing is claimed, so the row is left exactly as it was found and a
    // later fire can still escalate the block. Which fire that is, if any, is
    // not this use case's promise: the job is a one-shot.
    let recorder = Recorder {
        rows: vec![row("s1", NOW - WINDOW)],
        ..Default::default()
    };
    assert_eq!(
        fired(&recorder, &away()),
        StaleOutcome::Held {
            waiting: 1,
            why: Suppressed::Away
        }
    );
    assert!(recorder.claimed.borrow().is_empty());
    assert!(recorder.pages.borrow().is_empty());
}

#[test]
fn a_row_another_fire_already_claimed_is_never_paged_about() {
    let recorder = Recorder {
        rows: vec![row("s1", NOW - WINDOW), row("s2", NOW - WINDOW)],
        refuse: "s1",
        ..Default::default()
    };
    assert_eq!(fired(&recorder, &at_the_desk()), StaleOutcome::Paged(1));
    assert_eq!(*recorder.claimed.borrow(), ["s1", "s2"]);
    assert_eq!(
        recorder
            .pages
            .borrow()
            .iter()
            .map(|page| page.session.clone())
            .collect::<Vec<_>>(),
        ["s2"]
    );
}

#[test]
fn a_window_of_zero_reads_no_rows_at_all() {
    let recorder = Recorder {
        rows: vec![row("s1", 0)],
        ..Default::default()
    };
    let outcome = EscalateStaleBlocks {
        waits: &recorder,
        notifier: &recorder,
    }
    .run(NOW, 0, "", &at_the_desk());
    assert_eq!(outcome, StaleOutcome::Off);
    assert!(recorder.claimed.borrow().is_empty());
    assert!(recorder.pages.borrow().is_empty());
}
