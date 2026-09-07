/// What one failed lane's alert says: the count, and the last thing that went
/// wrong.
pub fn alert_summary(lane: &crate::LaneReport) -> String {
    // THE LAST FAILURE, not the last line. The card is read on a phone and has
    // room for one sentence, and a lane KEEPS GOING after a failure: its final
    // line is routinely a later success, so `1 failure(s); plugin a:
    // refreshed` is an alert that names nothing to fix. The rest is in the
    // record.
    match lane.last_failure() {
        Some(failure) => format!("{} failure(s); {failure}", lane.failures()),
        None => format!("{} failure(s)", lane.failures()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LaneReport;

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
}
