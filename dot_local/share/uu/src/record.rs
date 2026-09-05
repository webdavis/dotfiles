//! The records path: one entry per run, posted straight to the hermes
//! gateway.
//!
//! THREE QUESTIONS, three files. This one composes THE ENTRY: what the run
//! amounted to, the detail a human reads, and the body the gateway receives.
//! `marker` owns the last-successful-run timestamp and the gap sentence every
//! entry opens with; `event` owns the JSON a command lane's child is handed.

mod event;
mod marker;

pub use event::{RunFacts, lane_event};
pub use marker::{Marker, elapsed, gap_line, marker_contents, parse_marker};

use crate::lanes::LaneReport;

/// The agent name every uu record and alert carries.
pub const AGENT: &str = "uu";

/// The record's `state` field: what the whole run amounted to.
///
/// ONE FAILURE ANYWHERE MAKES THE RUN FAILED, which wins over a deferral in
/// the same run: the record is read at a glance, and a partial success
/// reported as a success is exactly the reading the record exists to
/// prevent. With no failure, a run that DEFERRED at least one lane is its
/// own state rather than "completed": nothing happened is a different week
/// from nothing needed to happen, and collapsing the two into "completed"
/// is exactly the reading a deferral must never get.
pub fn record_state(failures: usize, deferred: usize) -> &'static str {
    if failures > 0 {
        "failed"
    } else if deferred > 0 {
        "deferred"
    } else {
        "completed"
    }
}

/// The record's `detail`: the header, the gap, every lane's own lines, and the
/// count that closes it.
///
/// A RUN THAT RAN NO LANE SAYS SO. An entry listing nothing reads identically
/// to an entry whose lanes all passed quietly, and those are very different
/// weeks: one of them is a config that turned everything off.
///
/// A DEFERRED LANE IS NAMED `deferred`, never `0 failure(s)`: the two read
/// identically to a lane that ran clean, and "nothing happened" is not the
/// same week as "nothing needed to happen".
pub fn record_detail(host: &str, now_iso: &str, gap: &str, lanes: &[LaneReport]) -> String {
    let mut out = format!("run at {now_iso} on {host}\n{gap}\n");
    if lanes.is_empty() {
        out.push_str("no lane is enabled in this config, so nothing was updated\n");
    }
    let mut failures = 0;
    let mut deferred = 0;
    for lane in lanes {
        failures += lane.failures;
        let verdict = if lane.deferred {
            deferred += 1;
            "deferred".to_string()
        } else {
            format!("{} failure(s)", lane.failures)
        };
        out.push_str(&format!("\n{}: {verdict}\n", lane.name));
        for line in &lane.lines {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str(&format!(
        "\n=== done, {failures} failure(s), {deferred} deferred ===\n"
    ));
    out
}

/// uu's own gateway body. The four field NAMES are the hermes webhook's
/// contract; what goes in them is uu's.
///
/// BUILT BY THE JSON WRITER, never by interpolation. Every value in here is
/// text a third party wrote (a plugin name, a herdr error), and a quote in one
/// of them would otherwise end the field early.
pub fn record_body(state: &str, host: &str, detail: &str) -> String {
    serde_json::json!({
        "agent": AGENT,
        "state": state,
        "project": host,
        "detail": detail,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(name: &str, failures: usize, lines: &[&str]) -> LaneReport {
        LaneReport {
            name: name.to_string(),
            failures,
            deferred: false,
            lines: lines.iter().map(|line| line.to_string()).collect(),
            last_failure: None,
        }
    }

    fn deferred_report(name: &str, lines: &[&str]) -> LaneReport {
        LaneReport {
            name: name.to_string(),
            failures: 0,
            deferred: true,
            lines: lines.iter().map(|line| line.to_string()).collect(),
            last_failure: None,
        }
    }

    // --- the run's own state ---------------------------------------------------

    #[test]
    fn a_clean_run_is_completed_and_any_failure_makes_the_whole_run_failed() {
        assert_eq!(record_state(0, 0), "completed");
        assert_eq!(record_state(1, 0), "failed");
        assert_eq!(record_state(9, 0), "failed");
    }

    #[test]
    fn a_run_with_no_failure_but_a_deferral_is_its_own_state_not_completed() {
        // "nothing happened" and "nothing needed to happen" are different
        // weeks, and reporting the first as "completed" is exactly the
        // reading a deferral must never get.
        assert_eq!(record_state(0, 1), "deferred");
        assert_eq!(record_state(0, 3), "deferred");
    }

    #[test]
    fn a_failure_anywhere_wins_over_a_deferral_in_the_same_run() {
        assert_eq!(record_state(1, 1), "failed");
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
                .ends_with("=== done, 5 failure(s), 0 deferred ==="),
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
                .ends_with("=== done, 2 failure(s), 2 deferred ==="),
            "{detail}"
        );
    }

    // --- the body -------------------------------------------------------------

    #[test]
    fn the_body_carries_uus_own_four_fields_and_nothing_else() {
        let body = record_body("completed", "dresden", "the whole record");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["agent"], "uu");
        assert_eq!(parsed["state"], "completed");
        assert_eq!(parsed["project"], "dresden");
        assert_eq!(parsed["detail"], "the whole record");
        assert_eq!(parsed.as_object().unwrap().len(), 4);
    }

    #[test]
    fn a_detail_holding_json_syntax_is_encoded_rather_than_glued_into_the_body() {
        let body = record_body("failed", "dresden", "plugin \"a\": {broken}\nnext");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["detail"], "plugin \"a\": {broken}\nnext");
    }
}
