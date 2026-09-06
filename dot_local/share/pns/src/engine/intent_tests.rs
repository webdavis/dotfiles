//! The decision, pinned: intent.

use super::fixtures::*;

// --- caller intent ------------------------------------------------------

#[test]
fn skip_phone_beats_force_phone_because_already_sent_is_more_specific() {
    let probes = CountingProbes::default();
    let overrides = Overrides {
        skip_phone: true,
        force_phone: true,
        ..Overrides::default()
    };
    assert!(!names(&decide_with(&probes, &overrides, "")).contains(&"mobile"));
}

#[test]
fn one_decision_reads_each_probe_at_most_once_and_never_twice() {
    // State at the last moment before delivery, taken ONCE (operator
    // ruling 2026-08-13). A probe consulted a second time could answer
    // differently, and one decision would then be split between two
    // readings of where the operator is.
    let probes = CountingProbes {
        idle: Some(1),
        marker_mtime: Some(999_000),
        phone_atime: Some(999_900),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    decide_with(&probes, &Overrides::default(), "wW:p1");
    for (reads, probe) in [
        (probes.idle_reads.get(), "idle"),
        (probes.marker_reads.get(), "marker"),
        (probes.phone_reads.get(), "phone input"),
        (probes.view_reads.get(), "session view"),
    ] {
        assert!(reads <= 1, "the {probe} probe was read {reads} times");
    }
    // And the view really was consulted, so the bound above is a bound
    // rather than a probe that never ran.
    assert_eq!(probes.view_reads.get(), 1);
}

#[test]
fn force_phone_sends_the_card_from_the_desk_with_the_pane_in_plain_sight() {
    // The override outranks the surface entirely, which is what moshi-gate
    // and the hooks rely on.
    let probes = CountingProbes {
        idle: Some(1),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    let overrides = Overrides {
        force_phone: true,
        ..Overrides::default()
    };
    assert!(names(&decide_with(&probes, &overrides, "wW:p1")).contains(&"mobile"));
}

#[test]
fn a_stated_phone_input_age_spares_the_process_walk_behind_it() {
    // The reading costs three spawns and a walk over live processes, so
    // a caller who already stated the answer must never pay for it.
    let probes = CountingProbes {
        idle: Some(9_000),
        phone_atime: Some(999_999),
        ..CountingProbes::default()
    };
    let overrides = Overrides {
        phone_input_age: Some(0),
        ..Overrides::default()
    };
    decide_with(&probes, &overrides, "wW:p1");
    assert_eq!(probes.phone_reads.get(), 0);
}

#[test]
fn a_garbage_phone_override_is_unknown_without_a_probe_read() {
    // Same rule as the idle override beside it: a present-but-garbled
    // value is refused rather than falling back to the live reading,
    // which would let a probe answer a question the caller overrode.
    let vars = BTreeMap::from([(
        "PNS_PHONE_INPUT_AGE".to_string(),
        "not-a-number".to_string(),
    )]);
    let overrides = Overrides::from_env(&vars);
    let probes = CountingProbes {
        idle: Some(9_000),
        phone_atime: Some(999_999),
        ..CountingProbes::default()
    };
    let decision = decide_with(&probes, &overrides, "");
    assert_eq!(probes.phone_reads.get(), 0);
    assert!(
        names(&decision).contains(&"mobile"),
        "an unknown phone reading falls toward away, which cards"
    );
}

#[test]
fn a_locked_screen_sends_a_blocked_approval_to_the_phone_rather_than_the_lock_screen() {
    // `operator_surface` is the approval gate: Desk means the harness
    // prompt already in front of the operator is the way to answer, and
    // anything else means the card is. A lock screen is not a prompt they
    // can answer, so the approval has to travel.
    let probes = CountingProbes {
        idle: Some(2),
        screen_locked: Some(true),
        ..CountingProbes::default()
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
    let probes = CountingProbes {
        idle: Some(2),
        screen_locked: Some(true),
        view: Some(elsewhere("wW:p1")),
        ..CountingProbes::default()
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
fn the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading() {
    // The lock's only job is to disqualify what the idle probe reported,
    // so taking it where that reading was never taken, or where it came
    // back empty, is a spawn for an answer nothing can use. The other
    // direction is the ruling: caller intent is never overridden, and
    // stating the desk clock states the desk's whole story, garbled value
    // included.
    let garbled = Overrides::from_env(&BTreeMap::from([(
        "PNS_IDLE_SECS".to_string(),
        "not-a-number".to_string(),
    )]));
    // (label, overrides, what the idle probe answers, idle reads, lock reads)
    let cases: [(&str, Overrides, Option<u64>, u32, u32); 4] = [
        (
            "nothing stated: the engine takes both readings",
            Overrides::default(),
            Some(2),
            1,
            1,
        ),
        (
            "a stated idle clock: it takes neither",
            Overrides {
                idle_secs: Some(9_000),
                ..Overrides::default()
            },
            Some(2),
            0,
            0,
        ),
        ("a garbled one: neither, again", garbled, Some(2), 0, 0),
        (
            "an unreadable idle clock: nothing arrived for the lock to disqualify",
            Overrides::default(),
            None,
            1,
            0,
        ),
    ];
    for (label, overrides, idle, idle_reads, lock_reads) in cases {
        let probes = CountingProbes {
            idle,
            screen_locked: Some(true),
            ..CountingProbes::default()
        };
        decide_with(&probes, &overrides, "");
        assert_eq!(probes.idle_reads.get(), idle_reads, "case: {label}, idle");
        assert_eq!(probes.lock_reads.get(), lock_reads, "case: {label}, lock");
    }
}

#[test]
fn an_overridden_idle_reading_spares_the_idle_probe() {
    let probes = CountingProbes {
        idle: Some(5),
        ..CountingProbes::default()
    };
    let overrides = Overrides {
        idle_secs: Some(9_000),
        ..Overrides::default()
    };
    decide_with(&probes, &overrides, "");
    assert_eq!(probes.idle_reads.get(), 0);
}

#[test]
fn a_phone_probe_that_read_nothing_leaves_the_operator_at_their_desk() {
    // The discovery chain walks live processes and any step can come back
    // empty. Reading that as "just used" would put the operator on a
    // phone that is not in their hand and silence the banner in front of
    // them, so no reading has to mean no phone.
    let probes = CountingProbes {
        idle: Some(2),
        phone_atime: None,
        view: Some(elsewhere("wW:p1")),
        ..CountingProbes::default()
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
    let probes = CountingProbes {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        phone_atime: Some(999_990),
        ..CountingProbes::default()
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
    let probes = CountingProbes {
        idle: Some(9_000),
        marker_mtime: Some(999_990),
        ..CountingProbes::default()
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
