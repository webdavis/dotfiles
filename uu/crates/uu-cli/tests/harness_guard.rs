//! The harness's own speed guard, pinned in one binary rather than in
//! every one that shares the harness.
//!
//! THE SAME TWINS as pns's copy of this guard: the pure predicates are
//! pinned with literal inputs rather than the constants they check, and the
//! two end to end tests backdate a real `Home`'s construction instant
//! instead of sleeping past it. The two backdated twins each print one
//! budget line to stderr on every run, by construction; the home name in
//! that line says "guard-twin".

mod support;

use std::time::Instant;
use support::*;
#[test]
fn a_fast_home_is_not_over_budget() {
    assert!(!over_budget(10));
}

#[test]
fn a_home_past_the_budget_is_over_budget() {
    assert!(over_budget(1_500));
}

#[test]
fn a_home_past_the_ceiling_with_no_excuse_is_over_ceiling() {
    assert!(over_ceiling(6_000, false, false));
}

#[test]
fn an_excused_home_is_never_over_ceiling() {
    assert!(!over_ceiling(6_000, true, false));
}

#[test]
fn an_already_panicking_thread_is_never_double_panicked() {
    assert!(!over_ceiling(6_000, false, true));
}

/// Drop a real home whose construction instant was pushed back by
/// `age_ms`, optionally excused, and say what its own drop panicked
/// with, if anything.
fn drop_backdated(name: &str, age_ms: u64, excuse: Option<&'static str>) -> Option<String> {
    let mut home = Home::new(name);
    home.created = Instant::now() - std::time::Duration::from_millis(age_ms);
    if let Some(reason) = excuse {
        home.allow_slow(reason);
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(home)))
        .err()
        .map(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default()
        })
}

#[test]
fn a_real_home_past_the_ceiling_fails_naming_the_test_budget() {
    let message = drop_backdated("guard-twin-ceiling", TEST_CEILING_MS as u64 + 1, None)
        .expect("a home over the ceiling must fail its own drop");
    assert!(message.starts_with("test budget:"), "{message}");
}

#[test]
fn a_real_home_past_the_ceiling_with_allow_slow_does_not_fail() {
    assert!(
        drop_backdated(
            "guard-twin-ceiling-excused",
            TEST_CEILING_MS as u64 + 1,
            Some("a structural reason, for this twin alone")
        )
        .is_none(),
        "allow_slow must lift the ceiling"
    );
}
