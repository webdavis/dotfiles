use super::*;
#[test]
fn acknowledged_legs_are_retained_and_never_leased_again() {
    let store = SqliteStore::new(state());
    let input = submission();
    let legs = created(&store, &input);
    for leg in &legs {
        store.record(&leg.claim, &acknowledged(), 11).unwrap();
    }
    assert!(
        store
            .claim_retry(lease(100, 110), Default::default())
            .unwrap()
            .is_none()
    );
    let history = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(history.attempts.len(), 2);
    assert!(
        history
            .attempts
            .iter()
            .all(|a| a.completion == acknowledged())
    );
    assert!(matches!(
        store.prepare(&input, lease(100, 110)).unwrap(),
        PreparedSubmission::Existing(_)
    ));
}
#[test]
fn retry_outcomes_keep_their_details_and_are_due_only_at_the_exact_retry_instant() {
    let store = SqliteStore::new(state());
    let _open = store.connect().unwrap();
    for outcome in [
        UnconfirmedDelivery::Failed,
        UnconfirmedDelivery::Unlaunched,
        UnconfirmedDelivery::Unknown,
    ] {
        for due in [30, 31] {
            let mut input = submission();
            input.identity.request_id = format!("{outcome:?}-{due}");
            let legs = created(&store, &input);
            let completion = LedgerCompletion::Retry {
                outcome,
                detail: "literal diagnostic\nnext".into(),
                retry_at: 30,
            };
            store.record(&legs[0].claim, &completion, 11).unwrap();
            store.record(&legs[1].claim, &acknowledged(), 12).unwrap();
            assert!(
                store
                    .claim_retry(lease(29, 40), Default::default())
                    .unwrap()
                    .is_none()
            );
            let record = store.inspect(&input.identity).unwrap().unwrap();
            assert_eq!(record.attempts[0].completion, completion);
            assert_eq!(record.attempts[0].at, 11);
            let retried = store
                .claim_retry(lease(due, 40), Default::default())
                .unwrap()
                .unwrap();
            assert_eq!(retried.leg, input.legs[0]);
            store.record(&retried.claim, &acknowledged(), due).unwrap();
        }
    }
}
#[test]
fn a_completed_generation_cannot_be_rewritten_and_a_late_uncontested_completion_can_settle() {
    let store = SqliteStore::new(state());
    let input = submission();
    let legs = created(&store, &input);
    store.record(&legs[0].claim, &acknowledged(), 30).unwrap();
    assert_eq!(
        store.record(&legs[0].claim, &retry(40), 31),
        Err(LedgerFailure::LostClaim)
    );
    assert_eq!(
        store.inspect(&input.identity).unwrap().unwrap().attempts[0].completion,
        acknowledged()
    );
}
#[test]
fn retry_claims_follow_event_sequence_and_retained_attempt_order_without_replacing_history() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    let first = created(&store, &input);
    input.identity.request_id = "later".into();
    created(&store, &input);
    store.record(&first[0].claim, &retry(20), 11).unwrap();
    store.record(&first[1].claim, &acknowledged(), 11).unwrap();
    let a = store
        .claim_retry(lease(20, 30), Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(a.identity.request_id, "original-id");
    store.record(&a.claim, &retry(40), 21).unwrap();
    let b = store
        .claim_retry(lease(22, 32), Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(b.identity.request_id, "later");
    let original = submission();
    let record = store.inspect(&original.identity).unwrap().unwrap();
    assert_eq!(record.attempts.len(), 3);
    assert_eq!(record.attempts[0].completion, retry(20));
    assert_eq!(record.attempts[2].completion, retry(40));
}
