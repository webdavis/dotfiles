use super::*;
#[test]
fn a_live_initial_lease_blocks_retries_until_its_exact_boundary_and_preserves_original_route() {
    for now in [19, 20, 21] {
        let store = SqliteStore::new(state());
        let input = submission();
        created(&store, &input);
        let claim = store
            .claim_retry(lease(now, now + 10), Default::default())
            .unwrap();
        if now == 19 {
            assert!(claim.is_none(), "live initial dispatch owns the leg");
            continue;
        }
        let claim = claim.unwrap();
        assert_eq!(claim.identity, input.identity);
        assert_eq!(claim.event, input.event);
        assert_eq!(claim.leg, input.legs[0]);
        let record = store.inspect(&input.identity).unwrap().unwrap();
        assert_eq!(record.attempts.last().unwrap().generation, 2);
    }
}
#[test]
fn invalid_or_overflow_edge_lease_windows_do_not_create_or_claim_rows() {
    let store = SqliteStore::new(state());
    let input = submission();
    for window in [lease(20, 19), lease(20, 20), lease(u64::MAX, u64::MAX)] {
        assert_eq!(
            store.prepare(&input, window).unwrap_err(),
            LedgerFailure::InvalidLease
        );
        assert_eq!(
            store.claim_retry(window, Default::default()).unwrap_err(),
            LedgerFailure::InvalidLease
        );
    }
    assert_eq!(store.inspect(&input.identity).unwrap(), None);
    let legs = store
        .prepare(&input, lease(u64::MAX - 1, u64::MAX))
        .unwrap();
    assert!(matches!(legs, PreparedSubmission::Created { .. }));
    assert!(
        store
            .claim_retry(lease(u64::MAX - 2, u64::MAX), Default::default())
            .unwrap()
            .is_none()
    );
}
#[test]
fn a_stale_generation_cannot_acknowledge_a_released_leg() {
    let store = SqliteStore::new(state());
    let input = submission();
    let legs = created(&store, &input);
    let retry = store
        .claim_retry(lease(20, 30), Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(
        store.record(&legs[0].claim, &acknowledged(), 21),
        Err(LedgerFailure::LostClaim)
    );
    store.record(&retry.claim, &acknowledged(), 22).unwrap();
    let history = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(history.attempts.len(), 3);
    assert_eq!(history.attempts[2].completion, acknowledged());
}
#[test]
fn a_claim_from_another_database_cannot_complete_a_matching_row() {
    let first = SqliteStore::new(state());
    let second = SqliteStore::new(state());
    let input = submission();
    let a = created(&first, &input);
    created(&second, &input);
    assert_eq!(
        second.record(&a[0].claim, &acknowledged(), 11),
        Err(LedgerFailure::LostClaim)
    );
    assert!(matches!(
        second.inspect(&input.identity).unwrap().unwrap().attempts[0].completion,
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Unknown,
            ..
        }
    ));
}
