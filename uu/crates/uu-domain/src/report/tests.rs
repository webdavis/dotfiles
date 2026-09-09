use super::*;

#[test]
fn a_lane_that_failed_without_saying_anything_still_produces_a_summary() {
    let report = LaneReport {
        name: "herdr".to_string(),
        failures: 1,
        verdict: LaneVerdict::Failed,
        lines: Vec::new(),
        last_failure: None,
    };
    assert_eq!(crate::alert_summary(&report), "1 failure(s)");
}

#[test]
fn a_pending_report_is_successful_work_but_is_not_completed() {
    let mut report = LaneReport::new("mine");
    report.pending("pins moved".into());
    report.noted("last detail".into());
    assert_eq!(report.verdict(), LaneVerdict::Pending);
    assert!(report.succeeded());
    assert_eq!(report.failures(), 0);
    assert_eq!(report.last_failure(), None);
    assert_eq!(report.lines, ["pins moved", "last detail"]);
}

#[test]
fn a_deferral_wins_over_pending_in_either_order() {
    for pending_first in [true, false] {
        let mut report = LaneReport::new("mine");
        if pending_first {
            report.pending("pending".into());
        }
        report.deferred("deferred".into());
        if !pending_first {
            report.pending("pending".into());
        }
        assert_eq!(report.verdict(), LaneVerdict::Deferred);
        assert!(!report.succeeded());
        assert_eq!(report.failures(), 0);
        assert_eq!(report.lines.len(), 2);
    }
}

#[test]
fn failure_wins_over_pending_and_deferral_in_every_order() {
    for actions in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut report = LaneReport::new("mine");
        for action in actions {
            match action {
                0 => report.failed("failure".into()),
                1 => report.deferred("deferred".into()),
                _ => report.pending("pending".into()),
            }
        }
        assert_eq!(report.verdict(), LaneVerdict::Failed);
        assert!(!report.succeeded());
        assert_eq!(report.failures(), 1);
        assert_eq!(report.last_failure(), Some("failure"));
        assert_eq!(report.lines.len(), 3);
    }
}
