//! The decision, pinned: intent.

use super::fixtures::{CountingProbes, decide_with, names, watching};
use pns_domain::Overrides;
use std::collections::BTreeMap;

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
