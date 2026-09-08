use super::{Submission, delivered_decision, event, missed_decision, run, submission};
use pns_domain::surface::Surface;
use pns_domain::{EventArgs, Overrides};

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

#[test]
fn an_original_class_exception_does_not_unmute_an_unmarked_return_summary() {
    let event = event();
    let mut decision = delivered_decision();
    decision.plan.pulse = false;
    for (muted, focus_active) in [(false, false), (true, false), (false, true), (true, true)] {
        let overrides = Overrides {
            muted,
            focus_active,
            ..Overrides::default()
        };
        let steps = run(submission(&event, &decision, &overrides));
        assert_eq!(
            steps.contains(&"replay".to_string()),
            !muted && !focus_active,
            "{steps:?}"
        );
        assert!(
            steps.contains(&"activity".to_string()),
            "original event is still recorded"
        );
        assert!(
            steps.contains(&"edge".to_string()),
            "presence still advances the return edge"
        );
    }
}
