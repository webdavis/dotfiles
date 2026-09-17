use super::*;

#[test]
fn attention_retains_occurrence_body_route_time_and_security_class() {
    let mut sut = subject(Status::Accepted, true);
    assert_eq!(sut.submit(&alert()), Submission::Accepted);
    let request = &sut.runner.requests[0];
    assert_eq!(request.producer.as_str(), "posture");
    assert_eq!(request.event.as_str(), "page");
    assert_eq!(request.signal, crate::wire::Signal::NeedsAttention);
    assert_eq!(request.occurred_at, Some(1730000000));
    assert_eq!(request.route.as_ref().unwrap().as_str(), "assigned-route");
    assert_eq!(request.class.as_ref().unwrap().as_str(), "security");
    assert_eq!(request.detail, "Security finding\nline one\nline two");
    assert_eq!(
        request.request_id.as_str(),
        "posture-d28d5af268c004d795ce0240f35f5218"
    );
}
#[test]
fn observations_stay_silent_and_do_not_invent_a_route() {
    let mut sut = subject(Status::Accepted, true);
    sut.route = None;
    let mut event = alert();
    event.event = "heartbeat";
    event.signal = AlertSignal::Observation;
    assert_eq!(sut.submit(&event), Submission::Accepted);
    let request = &sut.runner.requests[0];
    assert_eq!(request.signal, crate::wire::Signal::Observation);
    assert_eq!(request.class, None);
    assert_eq!(request.route, None);
}
#[test]
fn a_supplied_occurrence_is_stable_but_missing_occurrences_are_unique_per_call() {
    let mut sut = subject(Status::Accepted, true);
    let mut event = alert();
    for _ in 0..2 {
        assert_eq!(sut.submit(&event), Submission::Accepted);
    }
    assert_eq!(
        sut.runner.requests[0].request_id,
        sut.runner.requests[1].request_id
    );
    event.occurrence_id = None;
    for _ in 0..2 {
        assert_eq!(sut.submit(&event), Submission::Accepted);
    }
    assert_ne!(
        sut.runner.requests[2].request_id,
        sut.runner.requests[3].request_id
    );
}
#[test]
fn invalid_requests_are_refused_before_any_child_or_alarm_even_with_oversized_text() {
    for event in ["", "invalid\nevent"] {
        for detail in ["ordinary".to_string(), "x".repeat(300_000)] {
            let mut sut = subject(Status::Accepted, true);
            let mut input = alert();
            input.event = event;
            input.detail = detail;
            assert_eq!(
                sut.submit(&input),
                Submission::NotAccepted(SubmissionFailure::Refused)
            );
            assert!(sut.runner.requests.is_empty());
            assert!(sut.alarm.calls.is_empty());
        }
    }
}
#[test]
fn oversized_observations_remain_refused_without_a_security_notification() {
    let mut sut = subject(Status::Accepted, true);
    let mut input = alert();
    input.detail = "x".repeat(300_000);
    input.signal = AlertSignal::Observation;
    assert_eq!(
        sut.submit(&input),
        Submission::NotAccepted(SubmissionFailure::Refused)
    );
    assert!(sut.runner.requests.is_empty());
    assert!(sut.alarm.calls.is_empty());
}
#[test]
fn a_critical_finding_takes_the_route_its_tier_names_whatever_the_caller_configured() {
    let mut sut = subject(Status::Accepted, true);
    let mut input = alert();
    input.severity = Some(posture_domain::Severity::Critical);
    assert_eq!(sut.submit(&input), Submission::Accepted);
    // The tier names `priority` (posture_domain::severity_route), and the
    // producer spends the tier's route rather than the one it was built with.
    assert_eq!(
        sut.runner.requests[0].route.as_ref().unwrap().as_str(),
        "priority"
    );
}
#[test]
fn a_finding_below_critical_takes_the_route_the_caller_configured() {
    for tier in [
        posture_domain::Severity::Notice,
        posture_domain::Severity::Info,
    ] {
        let mut sut = subject(Status::Accepted, true);
        let mut input = alert();
        input.severity = Some(tier);
        assert_eq!(sut.submit(&input), Submission::Accepted);
        // No tier below critical names a route, so the one the caller was
        // configured with stands; on this machine that is `posture-pages`.
        assert_eq!(
            sut.runner.requests[0].route.as_ref().unwrap().as_str(),
            "assigned-route",
            "{tier:?}"
        );
    }
}
