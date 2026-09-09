use super::*;
#[test]
fn linear_retry_delays_preserve_exact_counts_and_saturate_unsigned_time() {
    let backoff = RetryBackoff { base_secs: 7 };
    assert_eq!(backoff.retry_at(10, 1), 17);
    assert_eq!(backoff.retry_at(10, 2), 24);
    assert_eq!(backoff.retry_at(u64::MAX - 5, 1), u64::MAX);
    assert_eq!(backoff.retry_at(10, u64::MAX), u64::MAX);
}

/// The jitter is gone, so the same arguments give the same answer every time.
/// Random spread exists to stop many clients retrying in one instant, and this
/// is one local daemon draining one queue against a loopback gateway: there is
/// no herd to spread, and the randomness only made tests nondeterministic.
#[test]
fn a_retry_time_is_a_pure_function_of_the_clock_and_the_attempt_count() {
    let backoff = RetryBackoff::default();
    let first = backoff.retry_at(1_000, 3);
    for _ in 0..8 {
        assert_eq!(backoff.retry_at(1_000, 3), first);
    }
    assert_eq!(first, 1_000 + 60 * 3);
}

/// Every code the design's tables name, in the class they name it in. A page
/// that will never be delivered has to stop on attempt one; a page that might
/// still arrive keeps its retries.
#[test]
fn every_classified_status_takes_the_class_the_design_gives_it() {
    for status in [400, 401, 403, 404, 405, 410, 422] {
        assert_eq!(
            DeliveryOutcome::Status(status).class(),
            FailureClass::Permanent,
            "status {status}"
        );
    }
    for status in [408, 429, 500, 502, 503, 504] {
        assert_eq!(
            DeliveryOutcome::Status(status).class(),
            FailureClass::Temporary,
            "status {status}"
        );
    }
    assert_eq!(DeliveryOutcome::NoResponse.class(), FailureClass::Temporary);
    assert_eq!(DeliveryOutcome::NoStatus.class(), FailureClass::Permanent);
}

/// A malformed URL never reached the wire, so it carries no status, and it will
/// not heal by being sent again.
#[test]
fn a_request_that_never_reached_the_wire_is_permanent_although_it_has_no_status() {
    assert!(DeliveryOutcome::NoStatus.class().is_permanent());
    assert!(!DeliveryOutcome::NoResponse.class().is_permanent());
}

/// The classifier is total, so an unlisted code still answers. An unrecognized
/// 4xx is permanent because the server is saying the request itself is wrong;
/// an unrecognized 5xx is temporary because the server is claiming the fault is
/// its own.
#[test]
fn an_unlisted_code_falls_to_the_class_of_its_family() {
    for status in [402, 418, 451, 499] {
        assert_eq!(
            DeliveryOutcome::Status(status).class(),
            FailureClass::Permanent,
            "status {status}"
        );
    }
    for status in [501, 507, 599] {
        assert_eq!(
            DeliveryOutcome::Status(status).class(),
            FailureClass::Temporary,
            "status {status}"
        );
    }
}

/// A success is not a failure and is never classified as one. The retry loop
/// asks for a class only after delivery has already failed, and a classifier
/// that answered "permanent" for a 200 would dead-letter a delivered page.
#[test]
fn a_delivered_status_has_no_failure_class_at_all() {
    for status in [200, 201, 204, 299] {
        assert_eq!(
            DeliveryOutcome::Status(status).failure_class(),
            None,
            "status {status}"
        );
    }
    assert_eq!(
        DeliveryOutcome::Status(404).failure_class(),
        Some(FailureClass::Permanent)
    );
    // A redirect was never followed and never delivered, so it is a failure.
    assert_eq!(
        DeliveryOutcome::Status(301).failure_class(),
        Some(FailureClass::Permanent)
    );
}

/// The point of the split: a permanent refusal stops immediately instead of
/// consuming the twenty attempts a temporary one is allowed.
#[test]
fn a_permanent_outcome_deadletters_on_the_first_attempt_and_a_temporary_one_does_not() {
    let limits = RetryLimits::default();
    assert_eq!(
        limits.verdict(DeliveryOutcome::Status(404), 1, 0, 0),
        Some(DeadletterReason::Permanent)
    );
    assert_eq!(limits.verdict(DeliveryOutcome::Status(503), 1, 0, 0), None);
    assert_eq!(
        limits.verdict(DeliveryOutcome::Status(503), 20, 0, 0),
        Some(DeadletterReason::Attempts)
    );
    assert_eq!(
        limits.verdict(DeliveryOutcome::Status(503), 1, 1, 604_802),
        Some(DeadletterReason::Age)
    );
}

/// Permanence outranks both existing limits, so the recorded reason names what
/// actually stopped the leg rather than whichever bound was crossed at the same
/// moment.
#[test]
fn a_permanent_refusal_outranks_the_attempt_and_age_limits_in_the_recorded_reason() {
    let limits = RetryLimits::default();
    assert_eq!(
        limits.verdict(DeliveryOutcome::Status(404), 99, 1, 604_802),
        Some(DeadletterReason::Permanent)
    );
}

/// A delivered outcome is never dead-lettered, whatever the counters say.
#[test]
fn a_delivered_outcome_is_never_deadlettered_even_past_every_limit() {
    let limits = RetryLimits::default();
    assert_eq!(
        limits.verdict(DeliveryOutcome::Status(200), 99, 1, 604_802),
        None
    );
}

#[test]
fn the_existing_exhaustion_rule_is_unchanged_for_callers_that_have_no_outcome() {
    let limits = RetryLimits::default();
    assert_eq!(limits.exhausted(20, 0, 0), Some(DeadletterReason::Attempts));
    assert_eq!(limits.exhausted(1, 1, 604_802), Some(DeadletterReason::Age));
    assert_eq!(limits.exhausted(1, 0, 0), None);
}
