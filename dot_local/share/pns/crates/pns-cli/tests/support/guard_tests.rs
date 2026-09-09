//! The guard's own twins. The pure predicates are pinned with literal
//! inputs rather than the constants they check, so a mutated constant
//! cannot also move what a twin calls "past the line"; that is why the
//! resolved-ceiling twins say 5_000 and 20_000 outright. The end to end
//! twins backdate a real sandbox's construction instant instead of
//! sleeping past it, so proving the wiring works costs no real wall
//! clock either, and they backdate against `live_ceiling_ms` rather than
//! a literal, so they pin the same line `Drop` reads on whichever
//! machine is running them. Each of them prints one budget line to
//! stderr on every run, by construction (a backdate big enough to reach
//! the ceiling is past the budget); the sandbox name in that line says
//! "guard-twin".
use super::Sandbox;
use super::budget::{ceiling_ms, live_ceiling_ms, over_budget, over_ceiling};
use std::time::Instant;

#[test]
fn a_fast_sandbox_is_not_over_budget() {
    assert!(!over_budget(10));
}

#[test]
fn a_sandbox_past_the_budget_is_over_budget() {
    assert!(over_budget(1_500));
}

#[test]
fn a_sandbox_past_the_ceiling_with_no_excuse_is_over_ceiling() {
    assert!(over_ceiling(6_000, 5_000, false, false));
}

#[test]
fn an_excused_sandbox_is_never_over_ceiling() {
    assert!(!over_ceiling(6_000, 5_000, true, false));
}

#[test]
fn an_already_panicking_thread_is_never_double_panicked() {
    assert!(!over_ceiling(6_000, 5_000, false, true));
}

/// The line is the longest life still ALLOWED, so a reading that ties it
/// passes and the next millisecond does not. Checked from both sides,
/// because equality is the one case a `>` and a `>=` disagree on.
#[test]
fn a_sandbox_exactly_on_the_ceiling_is_not_over_it() {
    assert!(!over_ceiling(5_000, 5_000, false, false));
}

#[test]
fn a_sandbox_one_ms_past_the_ceiling_is_over_it() {
    assert!(over_ceiling(5_001, 5_000, false, false));
}

#[test]
fn no_ci_signal_resolves_the_local_ceiling() {
    assert_eq!(ceiling_ms(None), 5_000);
}

/// An exported-but-empty `CI` is what a shell leaves behind, not a
/// signal, so it resolves the same line a developer's machine gets.
#[test]
fn an_empty_ci_variable_is_not_a_ci_run() {
    assert_eq!(ceiling_ms(Some("")), 5_000);
}

#[test]
fn a_ci_run_resolves_a_ceiling_four_times_the_local_one() {
    assert_eq!(ceiling_ms(Some("1")), 20_000);
}

/// GitHub Actions exports `CI=true` and this repository's own tooling
/// exports `CI=1`, so the signal is any non-empty value rather than one
/// spelling.
#[test]
fn any_non_empty_ci_value_is_a_ci_run() {
    assert_eq!(ceiling_ms(Some("true")), 20_000);
}

/// The one twin that pins the ENVIRONMENT READ rather than the
/// arithmetic, so a `live_ceiling_ms` that stopped consulting `CI` is not
/// silently the old bug back. The expectation is spelled out in literals
/// instead of calling `ceiling_ms`, which would only prove the function
/// agrees with itself. IT ONLY DISCRIMINATES WHERE `CI` IS SET, which is
/// CI, which is the environment the guard was getting wrong.
#[test]
fn the_live_ceiling_follows_this_process_environment() {
    let expected = match std::env::var("CI").ok().as_deref() {
        Some(value) if !value.is_empty() => 20_000,
        _ => 5_000,
    };
    assert_eq!(live_ceiling_ms(), expected);
}

/// THE READING THAT WAS FAILING CI: 5.5 s, measured on the runner for a
/// test that takes 1.3 s here. Past the local line, inside the CI one,
/// which is the whole behaviour this guard gained.
#[test]
fn a_sandbox_over_the_local_line_is_still_inside_the_ci_one() {
    assert!(over_ceiling(5_500, ceiling_ms(None), false, false));
    assert!(!over_ceiling(5_500, ceiling_ms(Some("1")), false, false));
}

/// Drop a real sandbox whose construction instant was pushed back by
/// `age_ms`, optionally excused, and say what its own drop panicked
/// with, if anything.
fn drop_backdated(name: &str, age_ms: u64, excuse: Option<&'static str>) -> Option<String> {
    let mut sandbox = Sandbox::without_config(name);
    sandbox.created = Instant::now() - std::time::Duration::from_millis(age_ms);
    if let Some(reason) = excuse {
        sandbox.allow_slow(reason);
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(sandbox)))
        .err()
        .map(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default()
        })
}

#[test]
fn a_real_sandbox_past_the_ceiling_fails_naming_the_test_budget() {
    let message = drop_backdated("guard-twin-ceiling", live_ceiling_ms() as u64 + 1, None)
        .expect("a sandbox over the ceiling must fail its own drop");
    assert!(message.starts_with("test budget:"), "{message}");
    assert!(
        message.contains(&format!("over the {} ms ceiling", live_ceiling_ms())),
        "the message must name the line actually in force: {message}"
    );
}

/// The OTHER side of the line, on the real `Drop` path: a sandbox inside
/// the ceiling in force must survive its own drop. It is the only twin
/// that fails a `Drop` reading the LOCAL constant while running in CI,
/// which is the defect this guard was carrying.
///
/// THE MARGIN IS A SECOND, NOT A MILLISECOND. The drop path does real
/// work between the backdate and the reading (a `remove_dir_all`, plus
/// whatever the scheduler charges a thread in a loaded test binary), so a
/// one-millisecond margin is a wall-clock race: measured here, a 4,999 ms
/// backdate already reads 5,000 ms under load, and 3 ms of runner
/// slowness turns it red. That is the failure this whole guard was
/// changed to stop having, so the twin must not reintroduce it one level
/// down. The exact boundary is pinned by the pure twins instead, which
/// spend no wall clock to do it.
#[test]
fn a_real_sandbox_well_inside_the_ceiling_does_not_fail() {
    assert!(
        drop_backdated(
            "guard-twin-inside-ceiling",
            live_ceiling_ms() as u64 - 1_000,
            None
        )
        .is_none(),
        "a sandbox inside the ceiling must not fail its own drop"
    );
}

#[test]
fn a_real_sandbox_past_the_ceiling_with_allow_slow_does_not_fail() {
    assert!(
        drop_backdated(
            "guard-twin-ceiling-excused",
            live_ceiling_ms() as u64 + 1,
            Some("a structural reason, for this twin alone")
        )
        .is_none(),
        "allow_slow must lift the ceiling"
    );
}
