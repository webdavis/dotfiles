//! The alerts path: a failed lane, said out loud through the pns engine.
//!
//! ALERTS ARE ARGV, NOT STDIN. pns's client interface is flags, the same ones
//! the shell notifier and the weekly bash jobs use; piping an event at it
//! yields an empty event and exit 0. uu is a pns CLIENT here and nothing more:
//! it never decides presence, escalation or which destination fires, because
//! that is the engine's whole job.
//!
//! NO `--channel`, deliberately. pns's default route IS the alert route, and
//! the record path is where the quiet weekly entry goes. An alert that landed
//! on the record channel would be a failure nobody is paged about.
//!
//! FAIL OPEN. An absent or refusing engine is reported on stderr and the run
//! stays clean, because a notification must never fail the work it reports on.

/// The flags one alert is sent with. Pure, so what crosses the boundary is
/// decided where it can be read rather than inside a spawn.
pub fn alert_argv(host: &str, lane: &str, summary: &str) -> Vec<String> {
    [
        "--agent",
        crate::record::AGENT,
        "--state",
        "failed",
        "--project",
        host,
        "--detail",
        &format!("{lane}: {summary}"),
    ]
    .map(str::to_string)
    .to_vec()
}

/// The spawn seam: the engine, and the flags it is handed.
pub trait Alerter {
    /// `Err` carries why the alert did not go out, already fit to print.
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String>;
}

/// What one failed lane's alert says: the count, and the last thing that went
/// wrong.
pub fn alert_summary(lane: &crate::lanes::LaneReport) -> String {
    // THE LAST FAILURE, not the last line. The card is read on a phone and has
    // room for one sentence, and a lane KEEPS GOING after a failure: its final
    // line is routinely a later success, so `1 failure(s); plugin a:
    // refreshed` is an alert that names nothing to fix. The rest is in the
    // record.
    match lane.last_failure.as_ref() {
        Some(failure) => format!("{} failure(s); {failure}", lane.failures),
        None => format!("{} failure(s)", lane.failures),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::LaneReport;

    #[test]
    fn an_alert_names_uu_the_failure_the_host_and_the_lane() {
        let argv = alert_argv("dresden", "herdr", "2 failure(s)");
        assert_eq!(
            argv,
            vec![
                "--agent",
                "uu",
                "--state",
                "failed",
                "--project",
                "dresden",
                "--detail",
                "herdr: 2 failure(s)",
            ]
        );
    }

    #[test]
    fn an_alert_never_names_a_channel_because_the_default_route_is_the_alert_route() {
        assert!(
            !alert_argv("dresden", "herdr", "x")
                .iter()
                .any(|a| a == "--channel"),
            "an alert on the record route is a failure nobody is paged about"
        );
    }

    #[test]
    fn every_flag_is_followed_by_its_own_value() {
        // pns drops a value flag whose next token is another recognized flag,
        // which would silently strip whatever the pair carried.
        let argv = alert_argv("dresden", "herdr", "2 failure(s)");
        assert_eq!(argv.len() % 2, 0, "{argv:?}");
        for pair in argv.chunks(2) {
            assert!(pair[0].starts_with("--"), "{argv:?}");
            assert!(!pair[1].starts_with("--"), "{argv:?}");
        }
    }

    #[test]
    fn the_summary_counts_the_failures_and_carries_the_last_one() {
        let mut report = LaneReport::new("herdr");
        report.noted("herdr self-update: ok".to_string());
        report.failed("plugin a: REINSTALL FAILED twice".to_string());
        let summary = alert_summary(&report);
        assert!(summary.contains("1 failure(s)"), "{summary}");
        assert!(
            summary.contains("plugin a: REINSTALL FAILED twice"),
            "{summary}"
        );
    }

    #[test]
    fn a_later_success_never_stands_in_for_the_failure_being_alerted() {
        // The lane keeps going after a failure, so the last line it wrote is
        // usually a later success. An alert carrying that reads like a lane
        // that worked, on the one card the operator is paged with.
        let mut report = LaneReport::new("herdr");
        report.failed("herdr self-update FAILED (exit 1)".to_string());
        report.noted("plugin a: refreshed".to_string());
        let summary = alert_summary(&report);
        assert!(summary.contains("herdr self-update FAILED"), "{summary}");
        assert!(!summary.contains("refreshed"), "{summary}");
    }

    #[test]
    fn a_lane_that_failed_without_saying_anything_still_produces_a_summary() {
        let report = LaneReport {
            name: "herdr".to_string(),
            failures: 1,
            lines: Vec::new(),
            last_failure: None,
        };
        assert_eq!(alert_summary(&report), "1 failure(s)");
    }
}
