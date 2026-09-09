use super::*;

#[test]
fn unavailable_failed_and_timed_out_engines_each_attempt_one_independent_alarm() {
    for (error, failure) in [
        (
            InspectionFailure::Unavailable,
            SubmissionFailure::Unavailable,
        ),
        (InspectionFailure::Failed, SubmissionFailure::Failed),
        (InspectionFailure::TimedOut, SubmissionFailure::TimedOut),
    ] {
        let mut sut = subject(Status::Accepted, true);
        sut.runner.response = Err(error);
        assert_eq!(sut.submit(&alert()), Submission::NotAccepted(failure));
        assert_eq!(sut.runner.requests.len(), 1);
        assert_eq!(sut.alarm.calls.len(), 1);
        assert!(sut.alarm.calls[0].1.contains("Security finding"));
    }
}
#[test]
fn a_nonzero_engine_exit_cannot_be_accepted_even_with_a_valid_receipt() {
    let mut sut = subject(Status::Accepted, true);
    sut.runner.response.as_mut().unwrap().exit = 42;
    assert_eq!(
        sut.submit(&alert()),
        Submission::NotAccepted(SubmissionFailure::Failed)
    );
    assert_eq!(sut.alarm.calls.len(), 1);
}
#[test]
fn empty_garbage_and_multiple_receipts_attempt_one_alarm_without_retrying() {
    for bytes in [b"".as_slice(), b"garbage", b"{}\n{}"] {
        let mut sut = subject(Status::Accepted, true);
        sut.runner.matching = false;
        sut.runner.response.as_mut().unwrap().bytes = bytes.to_vec();
        assert_eq!(
            sut.submit(&alert()),
            Submission::NotAccepted(SubmissionFailure::Unparseable)
        );
        assert_eq!(sut.runner.requests.len(), 1);
        assert_eq!(sut.alarm.calls.len(), 1);
    }
}
#[test]
fn failed_independent_alarm_never_turns_engine_failure_into_acceptance() {
    let mut sut = subject(Status::Accepted, true);
    sut.runner.response = Err(InspectionFailure::Unavailable);
    sut.alarm.fail = true;
    assert_eq!(
        sut.submit(&alert()),
        Submission::NotAccepted(SubmissionFailure::Unavailable)
    );
    assert_eq!(sut.alarm.calls.len(), 1);
}

#[test]
fn correlated_exit_two_refusals_do_not_report_an_engine_outage() {
    for diagnostic in ["submission_conflict", "submission_plan_invalid", "invalid"] {
        let mut sut = subject(Status::Rejected, false);
        let output = sut.runner.response.as_mut().unwrap();
        let mut result = pns_protocol::decode_result(&output.bytes).unwrap();
        result.diagnostics = vec![diagnostic.into()];
        output.bytes = result.encode().unwrap().into_bytes();
        output.exit = 2;
        assert_eq!(
            sut.submit(&alert()),
            Submission::NotAccepted(SubmissionFailure::Refused)
        );
        assert!(sut.alarm.calls.is_empty());
    }
}

#[test]
fn explicit_submission_unavailable_reports_failure_but_degraded_storage_does_not() {
    for (status, exit, diagnostic, failure, alarms) in [
        (
            Status::Rejected,
            2,
            "submission_unavailable",
            SubmissionFailure::Unavailable,
            1,
        ),
        (
            Status::Rejected,
            0,
            "submission_unavailable",
            SubmissionFailure::Unavailable,
            1,
        ),
        (
            Status::Degraded,
            0,
            "ledger_unavailable",
            SubmissionFailure::NotCommitted,
            0,
        ),
    ] {
        let mut sut = subject(status, false);
        let output = sut.runner.response.as_mut().unwrap();
        let mut result = pns_protocol::decode_result(&output.bytes).unwrap();
        result.diagnostics = vec![diagnostic.into()];
        output.bytes = result.encode().unwrap().into_bytes();
        output.exit = exit;
        assert_eq!(sut.submit(&alert()), Submission::NotAccepted(failure));
        assert_eq!(sut.alarm.calls.len(), alarms);
    }
}

#[test]
fn malformed_and_uncorrelated_nonzero_results_remain_detectable_failures() {
    for malformed in [false, true] {
        let mut sut = subject(Status::Rejected, false);
        sut.runner.matching = false;
        let output = sut.runner.response.as_mut().unwrap();
        output.exit = 2;
        if malformed {
            output.bytes = b"not protocol output".to_vec();
        }
        assert_eq!(
            sut.submit(&alert()),
            Submission::NotAccepted(SubmissionFailure::Failed)
        );
        assert_eq!(sut.alarm.calls.len(), 1);
    }
}
