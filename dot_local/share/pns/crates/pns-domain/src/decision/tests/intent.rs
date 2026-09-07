//! The decision, pinned: intent.

use super::fixtures::{
    decide, decide_with, elsewhere, names, operator_surface, three_selection, watching,
};
use crate::surface::Surface;
use crate::{EnvironmentSnapshot, Overrides};

// --- caller intent ------------------------------------------------------
#[test]
fn skip_phone_beats_force_phone_because_already_sent_is_more_specific() {
    let probes = EnvironmentSnapshot::default();
    let overrides = Overrides {
        skip_phone: true,
        force_phone: true,
        ..Overrides::default()
    };
    assert!(!names(&decide_with(&probes, &overrides, "")).contains(&"mobile"));
}

#[test]
fn force_phone_sends_the_card_from_the_desk_with_the_pane_in_plain_sight() {
    // The override outranks the surface entirely, which is what moshi-gate
    // and the hooks rely on.
    let probes = EnvironmentSnapshot {
        idle: Some(1),
        view: Some(watching("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let overrides = Overrides {
        force_phone: true,
        ..Overrides::default()
    };
    assert!(names(&decide_with(&probes, &overrides, "wW:p1")).contains(&"mobile"));
}

#[test]
fn a_locked_screen_sends_a_blocked_approval_to_the_phone_rather_than_the_lock_screen() {
    // `operator_surface` is the approval gate: Desk means the harness
    // prompt already in front of the operator is the way to answer, and
    // anything else means the card is. A lock screen is not a prompt they
    // can answer, so the approval has to travel.
    let probes = EnvironmentSnapshot {
        idle: Some(2),
        screen_locked: Some(true),
        ..EnvironmentSnapshot::default()
    };
    assert_ne!(
        operator_surface(&probes, &Overrides::default(), Some(1_000_000)),
        Surface::Desk
    );
}

#[test]
fn a_locked_screen_cards_the_phone_and_leaves_the_desk_banner_unraised() {
    // THE SHIPPED BUG, end to end: a keyboard touched two seconds before
    // the lock holds the surface at Desk for the rest of the freshness
    // window, so the banner fires at a lock screen and no card reaches
    // the phone. Without the lock these exact readings banner, which is
    // what makes both halves of this test bite.
    let probes = EnvironmentSnapshot {
        idle: Some(2),
        screen_locked: Some(true),
        view: Some(elsewhere("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
    let legs = names(&decision);
    assert!(
        legs.contains(&"mobile"),
        "the card must reach them: {legs:?}"
    );
    assert!(
        !legs.contains(&"macos-banner"),
        "nobody is in front of the display: {legs:?}"
    );
}

#[test]
fn a_phone_probe_that_read_nothing_leaves_the_operator_at_their_desk() {
    // The discovery chain walks live processes and any step can come back
    // empty. Reading that as "just used" would put the operator on a
    // phone that is not in their hand and silence the banner in front of
    // them, so no reading has to mean no phone.
    let probes = EnvironmentSnapshot {
        idle: Some(2),
        phone_atime: None,
        view: Some(elsewhere("wW:p1")),
        ..EnvironmentSnapshot::default()
    };
    let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
    let legs = names(&decision);
    assert!(legs.contains(&"macos-banner"), "got {legs:?}");
    assert!(!legs.contains(&"mobile"), "got {legs:?}");
}

#[test]
fn an_unreadable_clock_ages_no_phone_signal_rather_than_treating_it_as_fresh() {
    // Without a clock neither the pty nor the tap has an age, so both
    // drop out of the arbitration instead of counting as the newest
    // signal forever.
    let probes = EnvironmentSnapshot {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        phone_atime: Some(999_990),
        ..EnvironmentSnapshot::default()
    };
    let decision = decide(
        &probes,
        &three_selection(),
        &Overrides::default(),
        false,
        false,
        "",
        None,
        false,
        false,
    );
    assert!(
        names(&decision).contains(&"mobile"),
        "away still cards; neither phone signal decided it"
    );
}

#[test]
fn an_unreadable_clock_ages_no_marker_rather_than_treating_it_as_fresh() {
    // Without a clock the tap has no age, so it drops out of the
    // arbitration instead of counting as the newest signal forever.
    let probes = EnvironmentSnapshot {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        ..EnvironmentSnapshot::default()
    };
    let decision = decide(
        &probes,
        &three_selection(),
        &Overrides::default(),
        false,
        false,
        "",
        None,
        false,
        false,
    );
    assert!(
        names(&decision).contains(&"mobile"),
        "away still cards; the tap simply did not decide it"
    );
}
