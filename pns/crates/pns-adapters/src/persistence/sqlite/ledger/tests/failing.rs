use super::*;
use pns_application::SubmissionDelivery;

/// One failing hermes leg per store, answering `status`.
fn failed(status: u16, request_id: &str) -> (SqliteStore, LedgerSubmission) {
    let store = SqliteStore::new(state());
    let mut input = remote_input();
    input.identity.request_id = request_id.into();
    let destinations = destinations(status);
    let delivery = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    delivery
        .submit(&input, None, lease(10, 40), &|| Some(10), &|_| {})
        .unwrap();
    (store, input)
}

/// A leg that failed and has not been acknowledged is what "not arriving"
/// means, and the row carries what a listing shows without a second query.
#[test]
fn a_failing_leg_reports_its_route_producer_and_answer() {
    let (store, _) = failed(503, "one");
    let failures = store.failing_legs(20).unwrap();
    assert_eq!(failures.len(), 1);
    let failure = &failures[0];
    assert_eq!(failure.destination, "hermes");
    assert_eq!(failure.route, "urgent");
    assert_eq!(failure.agent, "agent");
    assert_eq!(
        failure.outcome,
        pns_domain::retry::DeliveryOutcome::Status(503)
    );
    // The initial send consumes no retry, which is what the counted attempts in
    // the message have to agree with.
    assert_eq!(failure.retries, 0);
    assert!(!failure.deadlettered);
}

/// A leg that later succeeded is history, not a problem, and its
/// acknowledgement is what says so. A listing that kept it would send the
/// operator after a delivery that already arrived.
#[test]
fn an_acknowledged_leg_leaves_the_listing() {
    let store = SqliteStore::new(state());
    let input = remote_input();
    let claims = created(&store, &input);
    store
        .record(
            &claims[0].claim,
            &pns_domain::Delivery::Failed("first try".into()),
            11,
            Default::default(),
        )
        .unwrap();
    assert_eq!(store.failing_legs(20).unwrap().len(), 1);

    let retry = store
        .claim_retry(lease(12, 40), Default::default())
        .unwrap()
        .unwrap();
    store
        .record(
            &retry.claim,
            &pns_domain::Delivery::Delivered("gateway accepted".into()),
            13,
            Default::default(),
        )
        .unwrap();
    assert!(store.failing_legs(20).unwrap().is_empty());
}

/// Twenty covers a bad night without paging. The cap is what stops a listing
/// becoming the thing the operator has to scroll past.
#[test]
fn the_listing_is_capped_and_keeps_the_newest() {
    let store = SqliteStore::new(state());
    for index in 0..25u64 {
        let mut input = remote_input();
        input.identity.request_id = format!("event-{index}");
        let claims = created(&store, &input);
        store
            .record(
                &claims[0].claim,
                &pns_domain::Delivery::Failed(format!("try {index}")),
                100 + index,
                Default::default(),
            )
            .unwrap();
    }
    let failures = store.failing_legs(20).unwrap();
    assert_eq!(failures.len(), 20);
    // Newest first, so the most recent failure is the one at the top.
    assert_eq!(failures[0].failed_at, 124);
    assert_eq!(failures[19].failed_at, 105);
    assert!(
        failures
            .windows(2)
            .all(|pair| pair[0].failed_at >= pair[1].failed_at),
        "the listing is ordered newest first"
    );
}

/// The id a listing shows is the id the detail view takes, which is the whole
/// contract between the two commands.
#[test]
fn the_listed_id_reads_back_the_same_failure() {
    let (store, _) = failed(404, "one");
    let listed = store.failing_legs(20).unwrap().remove(0);
    assert_eq!(store.failing_leg(listed.id).unwrap(), Some(listed));
}

/// An id naming nothing failing answers None rather than an error, because a
/// leg that has since been acknowledged and one that never existed are the same
/// news to the reader: nothing there to chase.
#[test]
fn an_id_that_names_nothing_failing_is_absent_rather_than_an_error() {
    let (store, _) = failed(404, "one");
    assert_eq!(store.failing_leg(9_999).unwrap(), None);
    // An id past what the column can hold is absent too rather than a panic on
    // the cast.
    assert_eq!(store.failing_leg(u64::MAX).unwrap(), None);
}
