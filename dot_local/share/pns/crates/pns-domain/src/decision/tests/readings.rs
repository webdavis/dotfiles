//! The decision, pinned: readings.

use super::fixtures::{decide, decide_with, elsewhere, three_selection, watching};
use crate::surface::{DeliveryPlan, Surface, Visibility};
use crate::{DEFAULT_DESK_IDLE_SECS, EnvironmentSnapshot, Overrides};

// --- the readings the decision ran on ------------------------------------
#[test]
fn a_decision_reports_the_readings_its_surface_was_decided_from() {
    // THE RECORD IS THE READINGS THIS DECISION RAN ON, never a second
    // reading taken afterwards. Two readings of where the operator is can
    // disagree, and an explanation taken from the later one belongs to a
    // moment the decision never saw.
    let probes = EnvironmentSnapshot {
        idle: Some(30),
        marker_mtime: Some(999_400),
        phone_atime: Some(999_912),
        screen_locked: Some(false),
        view: Some(watching("wW:p1")),
    };
    let inputs = decide_with(&probes, &Overrides::default(), "wW:p1").inputs;
    assert_eq!(inputs.desk_input_age, Some(30));
    assert_eq!(
        inputs.phone_input_age,
        Some(88),
        "aged against the one clock read"
    );
    assert_eq!(inputs.marker_age, Some(600), "aged against that same read");
    assert_eq!(inputs.screen_locked, Some(false));
    assert_eq!(inputs.desk_fresh_secs, Some(DEFAULT_DESK_IDLE_SECS));
    // THE CLOCK ITSELF, and not only the ages taken against it. The two
    // above are aged inside the surface reading, so a decision that
    // carried out no clock at all still reports them; the epoch every
    // recorded line leads with comes from THIS field, and a `None` here
    // dates every entry `-` while the ages beside it look measured.
    assert_eq!(
        inputs.now_secs,
        Some(1_000_000),
        "the one clock read, carried out on the decision it was read for"
    );
    assert_eq!(
        inputs.surface,
        Surface::Desk,
        "and the verdict those readings produced"
    );
}

#[test]
fn a_decision_reports_both_the_sessions_visibility_and_the_one_the_plan_ran_on() {
    // DRILL D6 THROUGH THE RECORD. A Back Tap with moshi closed rewrites a
    // session-reported Visible to Hidden, and a record carrying only the
    // rewritten answer says the session hid the pane when the session said
    // the opposite. Both are kept, so the rewrite is visible as itself
    // rather than only in the card it produced.
    let tapped = EnvironmentSnapshot {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        view: Some(watching("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let inputs = decide_with(&tapped, &Overrides::default(), "wW:p1").inputs;
    assert_eq!(inputs.surface, Surface::Mobile);
    assert_eq!(inputs.session_visibility, Visibility::Visible);
    assert_eq!(inputs.visibility, Visibility::Hidden, "the D6 rewrite");

    // D5: moshi open on the pane, where the rewrite must never reach, so
    // the two answers agree and the difference above is the rewrite alone.
    let watching_it = EnvironmentSnapshot {
        idle: Some(9_000),
        phone_atime: Some(999_990),
        view: Some(watching("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let inputs = decide_with(&watching_it, &Overrides::default(), "wW:p1").inputs;
    assert_eq!(inputs.session_visibility, Visibility::Visible);
    assert_eq!(inputs.visibility, Visibility::Visible);
}

#[test]
fn a_decision_reports_the_plan_it_arbitrated_and_not_the_matrix_it_started_from() {
    // THE ARBITRATED PLAN IS THE VERDICT. The matrix would banner this
    // event and the long-running tier would pulse it; the operator's mute
    // is applied after both, and a record carrying the matrix's answer
    // would explain a card that never arrived by describing one that was
    // planned.
    let probes = || EnvironmentSnapshot {
        idle: Some(2),
        view: Some(elsewhere("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let long_event = |overrides: &Overrides| {
        decide(
            &probes(),
            &three_selection(),
            overrides,
            false,
            false,
            "wW:p1",
            Some(1_000_000),
            true,
            false,
        )
        .plan
    };
    assert_eq!(
        long_event(&Overrides::default()),
        DeliveryPlan {
            banner: true,
            phone_card: false,
            pulse: true,
        },
        "unmuted control: the matrix's own answer"
    );
    assert_eq!(
        long_event(&Overrides {
            muted: true,
            ..Overrides::default()
        }),
        DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        }
    );
}
