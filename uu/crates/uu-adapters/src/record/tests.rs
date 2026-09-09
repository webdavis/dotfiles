use super::*;

fn report(name: &str, failures: usize, lines: &[&str]) -> LaneReport {
    let mut report = LaneReport::new(name);
    for _ in 0..failures {
        report.failed(String::new());
    }
    report.lines = lines.iter().map(|line| line.to_string()).collect();
    report
}

fn deferred_report(name: &str, lines: &[&str]) -> LaneReport {
    let mut report = report(name, 0, lines);
    report.deferred(String::new());
    report.lines.pop();
    report
}

// --- the run's own state ---------------------------------------------------

#[test]
fn a_clean_run_is_completed_and_any_failure_makes_the_whole_run_failed() {
    assert_eq!(record_state(0, 0, 0), "completed");
    assert_eq!(record_state(1, 0, 0), "failed");
    assert_eq!(record_state(9, 0, 0), "failed");
}

#[test]
fn a_run_with_no_failure_but_a_deferral_is_its_own_state_not_completed() {
    // "nothing happened" and "nothing needed to happen" are different
    // weeks, and reporting the first as "completed" is exactly the
    // reading a deferral must never get.
    assert_eq!(record_state(0, 1, 0), "deferred");
    assert_eq!(record_state(0, 3, 0), "deferred");
}

#[test]
fn a_failure_anywhere_wins_over_a_deferral_in_the_same_run() {
    assert_eq!(record_state(1, 1, 0), "failed");
}

// --- the detail ------------------------------------------------------------

#[test]
fn the_detail_opens_with_when_this_run_started_and_the_gap_before_it() {
    let detail = record_detail(
        "dresden",
        "2026-08-11T12:00:00Z",
        "last successful run: NEVER RECORDED on this machine",
        &[],
    );
    let mut lines = detail.lines();
    assert_eq!(lines.next(), Some("run at 2026-08-11T12:00:00Z on dresden"));
    assert_eq!(
        lines.next(),
        Some("last successful run: NEVER RECORDED on this machine")
    );
}

#[test]
fn a_run_with_no_lane_enabled_says_so_instead_of_reading_as_a_quiet_week() {
    let detail = record_detail("dresden", "iso", "gap", &[]);
    assert!(detail.contains("no lane is enabled"), "{detail}");
}

#[test]
fn every_lane_contributes_its_name_its_verdict_and_its_own_lines() {
    let detail = record_detail(
        "dresden",
        "iso",
        "gap",
        &[report(
            "herdr",
            1,
            &["herdr self-update: ok", "plugin a: refreshed"],
        )],
    );
    assert!(detail.contains("herdr: 1 failure(s)"), "{detail}");
    assert!(detail.contains("herdr self-update: ok"), "{detail}");
    assert!(detail.contains("plugin a: refreshed"), "{detail}");
}

#[test]
fn the_detail_closes_with_the_total_across_every_lane() {
    let detail = record_detail(
        "dresden",
        "iso",
        "gap",
        &[report("herdr", 2, &[]), report("other", 3, &[])],
    );
    assert!(
        detail
            .trim_end()
            .ends_with("=== done, 5 failure(s), 0 deferred, 0 pending ==="),
        "{detail}"
    );
}

#[test]
fn a_deferred_lane_is_named_deferred_rather_than_zero_failures() {
    let detail = record_detail(
        "dresden",
        "iso",
        "gap",
        &[deferred_report(
            "mine",
            &["updater: deferred (exit 75: another run holds the lock)"],
        )],
    );
    assert!(detail.contains("mine: deferred"), "{detail}");
    assert!(!detail.contains("mine: 0 failure(s)"), "{detail}");
    assert!(
        detail.contains("another run holds the lock"),
        "the deferred lane's own explanation is still kept: {detail}"
    );
}

#[test]
fn the_closing_line_counts_deferred_lanes_separately_from_failures() {
    let detail = record_detail(
        "dresden",
        "iso",
        "gap",
        &[
            report("a", 2, &[]),
            deferred_report("b", &[]),
            deferred_report("c", &[]),
        ],
    );
    assert!(
        detail
            .trim_end()
            .ends_with("=== done, 2 failure(s), 2 deferred, 0 pending ==="),
        "{detail}"
    );
}

#[test]
fn a_run_with_nothing_failed_or_deferred_but_something_pending_is_pending() {
    assert_eq!(record_state(0, 0, 1), "pending");
    assert_eq!(record_state(0, 0, 3), "pending");
    assert_eq!(record_state(0, 0, 0), "completed");
}

#[test]
fn a_deferral_wins_over_a_pending_lane_in_the_same_run() {
    assert_eq!(record_state(0, 1, 1), "deferred");
    assert_eq!(record_state(1, 1, 1), "failed");
    assert_eq!(record_state(1, 0, 1), "failed");
}

#[test]
fn a_pending_lane_is_named_pending_rather_than_zero_failures() {
    let mut lane = LaneReport::new("mine");
    lane.pending("two pins waiting".into());
    let detail = record_detail("host", "iso", "gap", &[lane]);
    assert!(detail.contains("mine: pending"), "{detail}");
    assert!(detail.contains("two pins waiting"), "{detail}");
    assert!(!detail.contains("mine: 0 failure(s)"), "{detail}");
}

#[test]
fn the_closing_line_counts_pending_lanes_beside_failures_and_deferrals() {
    let mut pending = LaneReport::new("waiting");
    pending.pending("pins".into());
    let lanes = [
        report("failed", 2, &["bad"]),
        deferred_report("deferred", &["busy"]),
        pending,
    ];
    let detail = record_detail("host", "iso", "gap", &lanes);
    assert!(
        detail.ends_with("=== done, 2 failure(s), 1 deferred, 1 pending ===\n"),
        "{detail}"
    );
}
