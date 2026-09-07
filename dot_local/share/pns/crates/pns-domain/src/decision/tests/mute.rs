//! The decision, pinned: mute.

use super::fixtures::{decide, decide_with, elsewhere, names, three_selection, watching};
use crate::routing::{Leg, ReportMode};
use crate::surface::SessionView;
use crate::{EnvironmentSnapshot, Overrides};

// --- the operator's mute ------------------------------------------------
#[test]
fn a_muted_decision_keeps_the_durable_log_and_drops_every_decorative_leg() {
    // THE MUTE IS DECORATION ONLY. hermes is not a field of the delivery
    // plan (routing sends the durable log unconditionally), so the record
    // survives a mute STRUCTURALLY, which is what makes the mute lossless
    // and safe to fail open. The two rows are the desk's banner and the
    // phone's card, each of which fires in this exact scenario unmuted.
    let muted = Overrides {
        muted: true,
        ..Overrides::default()
    };
    for (label, idle) in [
        ("at the desk: the banner", Some(2)),
        ("away: the card", Some(9_000)),
    ] {
        let probes = EnvironmentSnapshot {
            idle,
            view: Some(elsewhere("wW:p1")),
            ..EnvironmentSnapshot::default()
        };
        assert_eq!(
            names(&decide_with(&probes, &Overrides::default(), "wW:p1")).len(),
            2,
            "unmuted control: {label} fires alongside the log"
        );
        assert_eq!(
            names(&decide_with(&probes, &muted, "wW:p1")),
            vec!["hermes"],
            "case: {label}"
        );
    }
}

#[test]
fn a_muted_decision_plans_no_pulse_even_for_a_long_running_event() {
    // THE LIGHTS ARE DECORATION TOO, and the pulse is not a leg, so
    // dropping the legs alone leaves the room flashing at an operator who
    // asked for quiet. Slice 7's `hue.quiet_hours` is a different gate and
    // is never consulted here: a muted event plans no pulse at all.
    let long_event = |overrides: &Overrides| {
        decide(
            &EnvironmentSnapshot {
                idle: Some(2),
                view: Some(elsewhere("wW:p1")),
                ..EnvironmentSnapshot::default()
            },
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
        .pulse
    };
    assert!(long_event(&Overrides::default()), "unmuted control");
    assert!(!long_event(&Overrides {
        muted: true,
        ..Overrides::default()
    }));
}

#[test]
fn the_mute_beats_a_forced_phone_card_because_a_producer_cannot_overrule_the_operator() {
    // ORDER IS THE WHOLE BEHAVIOR: the mute has to be applied AFTER the
    // skip-beats-force arbitration. Applying it before hands force the win
    // silently, which a plausible tidy would do, and `PNS_FORCE_PHONE` is
    // set by every producer that thinks its event is important.
    let probes = || EnvironmentSnapshot {
        idle: Some(1),
        view: Some(watching("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let forced = Overrides {
        force_phone: true,
        ..Overrides::default()
    };
    assert!(
        names(&decide_with(&probes(), &forced, "wW:p1")).contains(&"mobile"),
        "unmuted control: force still reaches the phone"
    );
    let forced_and_muted = Overrides {
        force_phone: true,
        muted: true,
        ..Overrides::default()
    };
    assert_eq!(
        names(&decide_with(&probes(), &forced_and_muted, "wW:p1")),
        vec!["hermes"],
        "a mute a producer can override is not a mute"
    );
}

#[test]
fn an_unmuted_decision_is_the_one_that_shipped_before_the_mute_existed() {
    // THE FALSE-POSITIVE DIRECTION, which is the one a mute gets wrong
    // silently: nobody notices a notification that still arrives, and
    // everybody notices one that does not. The expectations are WRITTEN
    // OUT rather than derived from a second call, so an over-eager mute
    // cannot move both sides of the comparison at once, and the whole
    // `Decision` is compared, so the leg MODES are pinned as well.
    // (label, idle, view, long running, legs, pulse)
    type Case = (
        &'static str,
        Option<u64>,
        Option<SessionView>,
        bool,
        Vec<&'static str>,
        bool,
    );
    let matrix: [Case; 6] = [
        (
            "desk, watching the pane",
            Some(2),
            Some(watching("wW:p1")),
            false,
            vec!["hermes"],
            false,
        ),
        (
            "desk, pane on another tab",
            Some(2),
            Some(elsewhere("wW:p1")),
            false,
            vec!["macos-banner", "hermes"],
            false,
        ),
        (
            "desk, view unreadable",
            Some(2),
            None,
            false,
            vec!["macos-banner", "hermes"],
            false,
        ),
        (
            "away, pane on screen",
            Some(9_000),
            Some(watching("wW:p1")),
            false,
            vec!["mobile", "hermes"],
            false,
        ),
        (
            "away, pane hidden",
            Some(9_000),
            Some(elsewhere("wW:p1")),
            false,
            vec!["mobile", "hermes"],
            false,
        ),
        (
            "away and long running: the lights ride on top",
            Some(9_000),
            Some(elsewhere("wW:p1")),
            true,
            vec!["mobile", "hermes"],
            true,
        ),
    ];
    for (label, idle, view, long_running, legs, pulse) in matrix {
        let probes = EnvironmentSnapshot {
            idle,
            view,
            ..EnvironmentSnapshot::default()
        };
        let unmuted = Overrides {
            muted: false,
            ..Overrides::default()
        };
        let decision = decide(
            &probes,
            &three_selection(),
            &unmuted,
            false,
            false,
            "wW:p1",
            Some(1_000_000),
            long_running,
            false,
        );
        assert_eq!(
            (decision.legs, decision.plan.pulse, decision.pane_dropped),
            (
                legs.iter()
                    .map(|name| Leg {
                        name,
                        mode: ReportMode::Silent,
                        // THE THREE-CHANNEL ROSTER, STATED: hermes is the
                        // durable log and shows the operator nothing;
                        // moshi is the phone and macos-banner this
                        // screen, and both do. A plan that mislabelled
                        // one fails here as well as in routing's own
                        // tests, which is the point of stating it.
                        decorative: *name != "hermes",
                    })
                    .collect::<Vec<Leg>>(),
                pulse,
                false,
            ),
            "case: {label}"
        );
    }
}
