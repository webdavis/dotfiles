use super::*;

#[test]
fn retry_budget_excludes_the_initial_send_and_counts_interrupted_claims() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    input.legs.truncate(1);
    created(&store, &input);
    for number in 1..=20 {
        assert!(
            store
                .claim_retry(
                    lease(20 + number * 10, 30 + number * 10),
                    Default::default()
                )
                .unwrap()
                .is_some(),
            "retry {number} remains allowed"
        );
    }
    assert!(
        store
            .claim_retry(lease(230, 240), Default::default())
            .unwrap()
            .is_none(),
        "twenty interrupted retries exhaust the budget"
    );
    assert_eq!(
        store
            .inspect(&input.identity)
            .unwrap()
            .unwrap()
            .attempts
            .len(),
        21
    );
}

#[test]
fn retry_age_uses_the_original_positive_epoch_and_a_strict_ceiling() {
    let store = SqliteStore::new(state());
    let mut input = submission();
    input.legs.truncate(1);
    created(&store, &input);
    let at_limit = 10 + 604800;
    let retry = store
        .claim_retry(lease(at_limit, at_limit + 1), Default::default())
        .unwrap()
        .expect("exact age limit remains allowed");
    store
        .record(&retry.claim, &super::retry(at_limit + 1), at_limit)
        .unwrap();
    assert!(
        store
            .claim_retry(lease(at_limit + 1, at_limit + 2), Default::default())
            .unwrap()
            .is_none(),
        "original age must not restart on retry"
    );
}

#[test]
fn retry_age_preserves_zero_future_and_full_unsigned_original_epochs() {
    for created_at in [0, 100, u64::MAX - 20] {
        let store = SqliteStore::new(state());
        let mut input = submission();
        input.legs.truncate(1);
        let PreparedSubmission::Created { legs, .. } = store
            .prepare(&input, lease(created_at, created_at + 10))
            .unwrap()
        else {
            panic!("new input");
        };
        store.record(&legs[0].claim, &retry(1), 1).unwrap();
        assert!(
            store
                .claim_retry(
                    lease(2, 12),
                    pns_domain::retry::RetryLimits {
                        max_attempts: 20,
                        max_age_secs: 0
                    }
                )
                .unwrap()
                .is_some()
        );
        let epoch: [u8; 8] = store
            .connect()
            .unwrap()
            .query_row(
                "SELECT started FROM ledger_attempts WHERE generation = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(u64::from_be_bytes(epoch), created_at);
    }
}
