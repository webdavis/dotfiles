//! The decision, pinned: plan.

use super::fixtures::*;

// --- the plan drives the legs -------------------------------------------

#[test]
fn every_surface_and_visibility_pair_dispatches_the_legs_its_row_planned() {
    // The engine's half of the matrix: the model decides banner and card,
    // and these are the LEGS that come out of it. hermes is the durable
    // log and rides every row, which is what makes the other two the
    // observable difference.
    // (label, desk input age, session view, the legs it must dispatch)
    type Case = (
        &'static str,
        Option<u64>,
        Option<SessionView>,
        Vec<&'static str>,
    );
    let matrix: [Case; 5] = [
        (
            "at the desk watching the pane: log only",
            Some(2),
            Some(watching("wW:p1")),
            vec!["hermes"],
        ),
        (
            "at the desk, pane on another tab: banner",
            Some(2),
            Some(elsewhere("wW:p1")),
            vec!["macos-banner", "hermes"],
        ),
        (
            "at the desk, view unreadable: banner, never suppressed on doubt",
            Some(2),
            None,
            vec!["macos-banner", "hermes"],
        ),
        (
            "away, pane on screen: the card still fires",
            Some(9_000),
            Some(watching("wW:p1")),
            vec!["mobile", "hermes"],
        ),
        (
            "away, pane hidden: card, and no banner for an empty room",
            Some(9_000),
            Some(elsewhere("wW:p1")),
            vec!["mobile", "hermes"],
        ),
    ];
    for (label, idle, view, expected) in matrix {
        let probes = CountingProbes {
            idle,
            view,
            ..CountingProbes::default()
        };
        assert_eq!(
            names(&decide_with(&probes, &Overrides::default(), "wW:p1")),
            expected,
            "case: {label}"
        );
    }
}

#[test]
fn a_phone_used_more_recently_than_the_desk_never_gets_a_banner() {
    // The property the matrix rests on: terminal-notifier is a desk
    // surface, and mobile is not the desk. The desk was touched 90s ago
    // and the phone 5s ago, which is drill D5's own scenario.
    let probes = CountingProbes {
        idle: Some(90),
        phone_atime: Some(999_995),
        view: Some(elsewhere("wW:p1")),
        ..CountingProbes::default()
    };
    let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
    let legs = names(&decision);
    assert!(!legs.contains(&"macos-banner"), "got {legs:?}");
    assert!(legs.contains(&"mobile"), "got {legs:?}");
}

#[test]
fn what_put_the_operator_on_mobile_decides_whether_the_watched_pane_suppresses() {
    // Both rows are on mobile with the origin pane reported as on screen,
    // and only the reason differs. Drill D6 (2026-08-19) found the first
    // one silent: the tap moved the surface, the desk display had the pane
    // focused for nobody, and mobile-plus-visible ate the card.
    // (label, marker mtime, phone pty atime, the legs it must dispatch)
    type Case = (&'static str, Option<u64>, Option<u64>, Vec<&'static str>);
    let matrix: [Case; 2] = [
        (
            "D6: tapped, moshi never opened, so nothing is being watched",
            Some(999_990),
            None,
            vec!["mobile", "hermes"],
        ),
        (
            "D5: moshi open on the pane, which is watching it for real",
            None,
            Some(999_990),
            vec!["hermes"],
        ),
    ];
    for (label, marker_mtime, phone_atime, expected) in matrix {
        let probes = CountingProbes {
            idle: Some(9_000),
            marker_mtime,
            phone_atime,
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        assert_eq!(
            names(&decide_with(&probes, &Overrides::default(), "wW:p1")),
            expected,
            "case: {label}"
        );
    }
}

#[test]
fn a_tap_with_moshi_closed_cards_even_when_the_session_view_cannot_be_read() {
    // The other half of the D6 row: an unreadable view already never
    // suppressed, and the tap must not turn that into a new way to.
    let probes = CountingProbes {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        view: None,
        ..CountingProbes::default()
    };
    let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
    let legs = names(&decision);
    assert!(legs.contains(&"mobile"), "got {legs:?}");
    assert!(!legs.contains(&"macos-banner"), "mobile never banners");
}

#[test]
fn the_long_running_tier_pulses_and_says_so_in_the_decision() {
    let probes = CountingProbes {
        idle: Some(2),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    let decision = decide(
        &probes,
        &three_selection(),
        &Overrides::default(),
        false,
        false,
        "wW:p1",
        Some(1_000_000),
        true,
        false,
    );
    assert!(
        decision.plan.pulse,
        "the lights ride on top of every long event"
    );
    assert_eq!(
        names(&decision),
        vec!["hermes"],
        "and change nothing else about a watched desk pane"
    );
}

#[test]
fn the_mobile_watch_card_toggle_adds_the_card_only_when_it_is_on() {
    let on_the_phone = || CountingProbes {
        idle: Some(9_000),
        phone_atime: Some(999_990),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    let with_toggle = |on: bool| {
        let probes = on_the_phone();
        let decision: Decision = decide(
            &probes,
            &three_selection(),
            &Overrides::default(),
            false,
            false,
            "wW:p1",
            Some(1_000_000),
            true,
            on,
        );
        decision.legs.iter().any(|leg| leg.name == "mobile")
    };
    assert!(!with_toggle(false), "default off: the pulse says it alone");
    assert!(with_toggle(true), "on: the card joins the pulse");
}
