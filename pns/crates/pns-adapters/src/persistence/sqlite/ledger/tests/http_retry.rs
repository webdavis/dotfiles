use super::*;
use pns_application::SubmissionDelivery;

#[test]
fn permanent_http_retry_retains_identity_metadata_and_history_without_acknowledgement() {
    for status in [401, 403, 404, 413] {
        let store = SqliteStore::new(state());
        let input = remote_input();
        let destinations = destinations(status);
        let delivery = SubmissionDelivery {
            ledger: &store,
            decisions: &store,
            destinations: &destinations,
        };
        delivery
            .submit(&input, None, lease(10, 40), &|| Some(10), &|_| {})
            .unwrap();
        assert_eq!(
            store.delivery_health().unwrap().deadlettered_legs,
            0,
            "initial send remains owned for retry"
        );
        delivery
            .retry(lease(100, 130), &|| Some(100), &|_| {})
            .unwrap()
            .unwrap();
        let health = store.delivery_health().unwrap();
        assert_eq!(
            health.deadlettered_legs, 1,
            "HTTP {status} must stop queued retries"
        );
        assert_eq!(health.pending_legs, 0);
        assert!(health.alarm_generation.is_some());
        assert!(
            store
                .claim_retry(lease(1000, 1030), Default::default())
                .unwrap()
                .is_none()
        );
        let record = store.inspect(&input.identity).unwrap().unwrap();
        assert_eq!(record.submission, input);
        assert_eq!(record.attempts.len(), 2);
        assert!(
            matches!(record.attempts.last().unwrap().completion, LedgerCompletion::Rejected { status: code, .. } if code == status)
        );
        assert!(
            !record
                .attempts
                .iter()
                .any(|a| matches!(a.completion, LedgerCompletion::Acknowledged { .. }))
        );
    }
}
#[test]
fn queued_retry_backoff_uses_the_retry_number_instead_of_the_lease_duration() {
    let store = SqliteStore::new(state());
    let input = remote_input();
    let destinations = destinations(503);
    let delivery = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    delivery
        .submit(&input, None, lease(10, 40), &|| Some(10), &|_| {})
        .unwrap();
    for (number, at) in [(1, 100), (2, 1000)] {
        delivery
            .retry(lease(at, at + 30), &|| Some(at), &|_| {})
            .unwrap()
            .unwrap();
        let record = store.inspect(&input.identity).unwrap().unwrap();
        let LedgerCompletion::Retry { retry_at, .. } = record.attempts.last().unwrap().completion
        else {
            panic!("retryable failure");
        };
        assert!(
            (at + 60 * number..=at + 60 * number + 60).contains(&retry_at),
            "retry {number} due {retry_at}"
        );
    }
}
#[test]
fn the_initial_send_does_not_delay_the_first_queued_retry() {
    let store = SqliteStore::new(state());
    let input = remote_input();
    let destinations = destinations(503);
    let delivery = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    delivery
        .submit(&input, None, lease(10, 40), &|| Some(10), &|_| {})
        .unwrap();
    assert!(
        store
            .claim_retry(lease(10, 40), Default::default())
            .unwrap()
            .is_some(),
        "the first queued retry is already due after the initial send"
    );
}

#[test]
fn terminal_completion_rolls_back_with_its_alarm_and_does_not_starve_a_later_leg() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    input.producer_request = Some("retained request metadata".into());
    let claims = created(&store, &input);
    store
        .record(
            &claims[1].claim,
            &pns_domain::Delivery::Delivered("acknowledged sibling".into()),
            11,
            Default::default(),
        )
        .unwrap();
    store
        .record(
            &claims[0].claim,
            &pns_domain::Delivery::Failed("initial".into()),
            11,
            Default::default(),
        )
        .unwrap();
    let retry = store
        .claim_retry(lease(12, 20), Default::default())
        .unwrap()
        .unwrap();
    let before = store.inspect(&input.identity).unwrap().unwrap();
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER refuse_terminal_alarm BEFORE UPDATE ON delivery_health BEGIN SELECT RAISE(ABORT, 'owned fixture'); END;").unwrap();
    let rejected = pns_domain::Delivery::Rejected {
        status: 403,
        detail: "the literal rejection".into(),
    };
    assert!(
        store
            .record(&retry.claim, &rejected, 13, Default::default())
            .is_err()
    );
    assert_eq!(store.inspect(&input.identity).unwrap().unwrap(), before);
    assert_eq!(store.delivery_health().unwrap().deadlettered_legs, 0);
    assert!(
        store
            .claim_retry(lease(14, 19), Default::default())
            .unwrap()
            .is_none()
    );
    connection
        .execute_batch("DROP TRIGGER refuse_terminal_alarm;")
        .unwrap();
    store
        .record(&retry.claim, &rejected, 15, Default::default())
        .unwrap();
    let mut later = remote_input();
    later.identity.request_id = "later".into();
    let later_claims = created(&store, &later);
    store
        .record(
            &later_claims[0].claim,
            &pns_domain::Delivery::Failed("initial".into()),
            15,
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        store
            .claim_retry(lease(15, 20), Default::default())
            .unwrap()
            .unwrap()
            .identity,
        later.identity
    );
    assert_eq!(
        store
            .inspect(&input.identity)
            .unwrap()
            .unwrap()
            .attempts
            .last()
            .unwrap()
            .completion,
        LedgerCompletion::Rejected {
            status: 403,
            detail: "the literal rejection".into()
        }
    );
    assert!(matches!(
        store.inspect(&input.identity).unwrap().unwrap().attempts[1].completion,
        LedgerCompletion::Acknowledged { .. }
    ));
    let status: u16 = connection
        .query_row(
            "SELECT http_status FROM ledger_legs WHERE deadlettered_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, 403);
}
#[test]
fn a_stale_terminal_result_cannot_replace_the_successor_or_erase_an_interrupted_retry() {
    let store = SqliteStore::new(state());
    let input = remote_input();
    let claims = created(&store, &input);
    store
        .record(
            &claims[0].claim,
            &pns_domain::Delivery::Failed("initial".into()),
            11,
            Default::default(),
        )
        .unwrap();
    let stale = store
        .claim_retry(lease(12, 13), Default::default())
        .unwrap()
        .unwrap();
    let current = store
        .claim_retry(lease(13, 20), Default::default())
        .unwrap()
        .unwrap();
    let before = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(
        store.record(
            &stale.claim,
            &pns_domain::Delivery::Rejected {
                status: 401,
                detail: "stale".into()
            },
            14,
            Default::default()
        ),
        Err(LedgerFailure::LostClaim)
    );
    assert_eq!(store.inspect(&input.identity).unwrap().unwrap(), before);
    assert_eq!(store.delivery_health().unwrap().deadlettered_legs, 0);
    store
        .record(
            &current.claim,
            &pns_domain::Delivery::Failed("retry two".into()),
            15,
            pns_domain::retry::RetryBackoff { base_secs: 7 },
        )
        .unwrap();
    assert!(
        store
            .claim_retry(lease(28, 40), Default::default())
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .claim_retry(lease(29, 40), Default::default())
            .unwrap()
            .is_some()
    );
}
