use super::*;
#[test]
fn preparation_commits_the_original_identity_payload_routes_and_unknown_attempts_before_returning()
{
    let path = state();
    let store = SqliteStore::new(path.clone());
    let input = submission();
    let legs = created(&store, &input);
    assert_eq!(legs.len(), 2);
    let record = SqliteStore::new(path)
        .inspect(&input.identity)
        .unwrap()
        .unwrap();
    assert_eq!(
        record.submission, input,
        "every rendered field and resolved route must survive"
    );
    assert_eq!(record.attempts.len(), 2);
    for (attempt, leg) in record.attempts.iter().zip(legs) {
        assert_eq!(attempt.destination, leg.leg.destination);
        assert_eq!(attempt.generation, 1);
        assert_eq!(attempt.at, 10);
        assert_eq!(
            attempt.completion,
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unknown,
                detail: String::new(),
                retry_at: 20
            }
        );
    }
}
#[test]
fn an_identical_inflight_submission_returns_history_without_a_second_dispatch_claim() {
    let store = SqliteStore::new(state());
    let input = submission();
    created(&store, &input);
    let PreparedSubmission::Existing(record) = store.prepare(&input, lease(11, 21)).unwrap() else {
        panic!("duplicate must not dispatch")
    };
    assert_eq!(record.submission, input);
    assert_eq!(record.attempts.len(), 2);
}
#[test]
fn conflicting_payload_or_any_resolved_leg_fact_is_refused_without_overwriting_the_original() {
    let store = SqliteStore::new(state());
    let original = submission();
    created(&store, &original);
    for change in 0..14 {
        let mut input = submission();
        match change {
            0 => input.event.agent.push('!'),
            1 => input.event.state.push('!'),
            2 => input.event.project.push('!'),
            3 => input.event.branch.push('!'),
            4 => input.event.detail.push('!'),
            5 => input.event.title.push('!'),
            6 => input.event.message.push('!'),
            7 => input.event.preview.push('!'),
            8 => input.event.pane.push('!'),
            9 => input.legs[0].destination.push('!'),
            10 => input.legs[0].route.push('!'),
            11 => input.legs[0].mode = ReportMode::Silent,
            12 => input.legs[0].decorative = true,
            _ => input.legs.reverse(),
        }
        assert_eq!(
            store.prepare(&input, lease(11, 21)).unwrap_err(),
            LedgerFailure::ConflictingSubmission,
            "changed field {change}"
        );
    }
    assert_eq!(
        store
            .inspect(&original.identity)
            .unwrap()
            .unwrap()
            .submission,
        original
    );
}
#[test]
fn distinct_producer_or_request_ids_keep_identical_content_as_separate_monotonic_events() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    let mut sequences = Vec::new();
    for index in 0..3 {
        if index == 1 {
            input.identity.producer = "uu".into()
        }
        if index == 2 {
            input.identity.request_id = "second-id".into()
        }
        let PreparedSubmission::Created { sequence, .. } =
            store.prepare(&input, lease(10, 20)).unwrap()
        else {
            panic!("distinct logical event")
        };
        sequences.push(sequence);
    }
    assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
}
#[test]
fn duplicate_destination_instances_are_refused_and_an_empty_plan_is_retained() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    input.legs.push(input.legs[0].clone());
    assert_eq!(
        store.prepare(&input, lease(10, 20)).unwrap_err(),
        LedgerFailure::InvalidPlan
    );
    assert_eq!(store.inspect(&input.identity).unwrap(), None);
    input.legs.clear();
    assert!(created(&store, &input).is_empty());
    assert_eq!(
        store.inspect(&input.identity).unwrap().unwrap().submission,
        input
    );
}
