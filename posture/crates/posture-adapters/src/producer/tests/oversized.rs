use super::*;

const OMITTED: &str = "Posture security alert omitted\nA security finding exceeded notification limits. The full alert was not submitted and remains unacknowledged. Inspect the originating posture check.";

#[test]
fn security_text_over_the_cap_submits_one_bounded_omission_but_refuses_the_original() {
    for (title, detail) in [
        ("Security finding".into(), "x".repeat(7984)),
        ("Security finding".into(), "é".repeat(300_000)),
        ("x".repeat(9000), "finding".into()),
    ] {
        let mut sut = subject(Status::Accepted, true);
        let mut input = alert();
        input.title = title;
        input.detail = detail;
        assert_eq!(
            sut.submit(&input),
            Submission::NotAccepted(SubmissionFailure::Refused)
        );
        assert_eq!(sut.runner.requests.len(), 1);
        let notice = &sut.runner.requests[0];
        assert_eq!(notice.detail, OMITTED);
        assert_eq!(notice.event.as_str(), "notification-omitted");
        assert_eq!(notice.signal, crate::wire::Signal::NeedsAttention);
        assert_eq!(notice.class.as_ref().unwrap().as_str(), "security");
        assert_eq!(notice.route.as_ref().unwrap().as_str(), "assigned-route");
        assert_eq!(notice.occurred_at, input.occurred_at);
        assert!(sut.alarm.calls.is_empty());
    }
}

#[test]
fn text_at_the_cap_retains_its_exact_bytes_and_normal_acceptance() {
    let mut sut = subject(Status::Accepted, true);
    let mut input = alert();
    input.detail = "é".repeat(7983);
    let expected = format!("{}\n{}", input.title, input.detail);
    assert_eq!(expected.chars().count(), 8000);
    assert_eq!(sut.submit(&input), Submission::Accepted);
    assert_eq!(sut.runner.requests[0].detail, expected);
}

#[test]
fn omission_retries_are_stable_and_cannot_acknowledge_the_original_identity() {
    let mut sut = subject(Status::Accepted, true);
    let mut input = alert();
    assert_eq!(sut.submit(&input), Submission::Accepted);
    input.detail = "x".repeat(9000);
    for _ in 0..2 {
        assert_eq!(
            sut.submit(&input),
            Submission::NotAccepted(SubmissionFailure::Refused)
        );
    }
    assert_eq!(sut.runner.requests.len(), 3);
    assert_ne!(
        sut.runner.requests[0].request_id,
        sut.runner.requests[1].request_id
    );
    assert_eq!(
        sut.runner.requests[1].request_id,
        sut.runner.requests[2].request_id
    );
}

#[test]
fn omission_engine_failure_alarms_once_with_only_bounded_text() {
    for error in [
        InspectionFailure::Unavailable,
        InspectionFailure::Failed,
        InspectionFailure::TimedOut,
    ] {
        let mut sut = subject(Status::Accepted, true);
        sut.runner.response = Err(error);
        let mut input = alert();
        input.detail = "private original text".repeat(9000);
        assert_eq!(
            sut.submit(&input),
            Submission::NotAccepted(SubmissionFailure::Refused)
        );
        assert_eq!(sut.runner.requests.len(), 1);
        assert_eq!(
            sut.alarm.calls,
            [("Posture notification engine failed".into(), OMITTED.into())]
        );
    }
}

#[test]
fn an_omission_receipt_does_not_change_normal_rejection_or_degradation_meaning() {
    for status in [Status::Rejected, Status::Degraded] {
        let mut sut = subject(status, true);
        let mut input = alert();
        input.detail = "x".repeat(9000);
        assert_eq!(
            sut.submit(&input),
            Submission::NotAccepted(SubmissionFailure::Refused)
        );
        assert_eq!(sut.runner.requests.len(), 1);
        assert!(sut.alarm.calls.is_empty());
    }
}
