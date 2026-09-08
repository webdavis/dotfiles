/// The REVIEW line: a sandbox alive longer than this at drop deserves a
/// look, so `Drop for Sandbox` prints a line to stderr naming the sandbox and
/// the elapsed ms, greppable as "test budget". Never fails the build on its
/// own; see `TEST_CEILING_MS`.
///
/// THIS IS A LOWER BOUND ON THE TEST, NOT AN UPPER BOUND ON THE CODE: the
/// guard measures the sandbox's own LIFETIME (construction to drop), so a
/// test that builds and drops several short-lived sandboxes in a loop
/// (`lamp_run`, dispatch.rs:1742; the `held_after` closure, dispatch.rs:8284)
/// is invisible to it no matter how many it makes, because no single one is
/// held past the line.
pub(super) const TEST_BUDGET_MS: u128 = 1_000;

/// The FAILURE line ON THIS MACHINE, calibrated from `--report-time
/// --ensure-time` evidence gathered 2026-09-01: under libtest's parallel
/// scheduler, wall time is contention, not cost. The worst legitimate
/// parallel reading measured here was ~1.19 s; the slowest STRUCTURAL
/// sandbox (the daemon lease test, excused with `allow_slow`) measured ~5.4 s
/// alone. 5 s hard-fails a real regression while giving contention a margin
/// neither reading is close to.
///
/// IT IS THE LOCAL LINE ONLY. The same paragraph used to say CI's runner has
/// fewer and slower cores and then apply this number there anyway, which is
/// what made the guard fail three unrelated pull requests in one day; see
/// `CI_CEILING_FACTOR` for what CI gets instead.
const TEST_CEILING_MS: u128 = 5_000;

/// What the local line is multiplied by when the suite runs in CI.
///
/// THIS IS ONE MEASUREMENT, NOT A CALIBRATION, and it should be read that
/// way. On 2026-09-02 a single test,
/// `a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing`,
/// took ~1.3 s here and ~5.5 s on the GitHub runner: a ratio of about 4, from
/// one test on one day. Applying that ratio to the ceiling gives CI roughly
/// the headroom the local line was given (5 s against a ~1.19 s worst local
/// reading), which is the most a single data point supports. A CI sandbox
/// that ever passes 20 s is evidence this number was wrong rather than a
/// reason to raise it again: re-measure the ratio first.
///
/// KNOWN COST, because 20 s sits ABOVE this harness's own 10 s deadlines
/// (`poll_until`, and the capture stream's read timeout): a test that
/// regresses into ONE hung poll spends ~10 s and now PASSES in CI, where it
/// still fails here. CI keeps catching two stacked ones, a real hang, and
/// anything structurally worse. That is the price of a gate that stopped
/// failing unrelated pull requests, and the single-poll case is still caught
/// by a local `just test-rust`.
const CI_CEILING_FACTOR: u128 = 4;

/// Whether a sandbox that lived `elapsed_ms` has earned the review warning.
/// Pulled out of `Drop` so a twin can pin the boundary without spending real
/// wall clock to get a sandbox there.
pub(super) fn over_budget(elapsed_ms: u128) -> bool {
    elapsed_ms > TEST_BUDGET_MS
}

/// The ceiling in force for a given `CI` value: the local line, or the local
/// line times `CI_CEILING_FACTOR` on a runner.
///
/// TAKES THE VALUE RATHER THAN READING THE ENVIRONMENT, so the twins pin both
/// sides with literals and neither of them has to be run twice under two
/// environments to mean anything. `CI` set to the empty string is not a CI
/// run: an exported-but-empty variable is what a shell leaves behind, while
/// GitHub Actions exports `CI=true` and this repository's own tooling
/// exports `CI=1`, so the signal is any non-empty value.
pub(super) fn ceiling_ms(ci: Option<&str>) -> u128 {
    match ci {
        Some(value) if !value.is_empty() => TEST_CEILING_MS * CI_CEILING_FACTOR,
        _ => TEST_CEILING_MS,
    }
}

/// The ceiling this process will actually apply, read from `CI` once at the
/// point of use. The single place the environment is consulted, so `Drop` and
/// the backdated twins cannot disagree about where the line is.
pub(super) fn live_ceiling_ms() -> u128 {
    ceiling_ms(std::env::var("CI").ok().as_deref())
}

/// Whether a sandbox that lived `elapsed_ms` fails the build: past the
/// `ceiling` in force, not excused, and not already unwinding (a panic
/// mid-panic aborts the process instead of failing one test).
///
/// THE COMPARISON IS STRICT. A sandbox landing exactly on the ceiling passes:
/// the line is the longest life still allowed, and a reading that ties it to
/// the millisecond is not evidence of a regression.
pub(super) fn over_ceiling(
    elapsed_ms: u128,
    ceiling: u128,
    excused: bool,
    panicking: bool,
) -> bool {
    elapsed_ms > ceiling && !excused && !panicking
}
