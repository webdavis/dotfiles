//! The decision, pinned: guard.

use super::fixtures::{CountingProbes, decide_with, names, watching};
use crate::ports::environment::Wants;
use pns_domain::Overrides;
use std::collections::BTreeMap;

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
