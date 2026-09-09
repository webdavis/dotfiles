use super::*;
use pns_application::SubmissionDelivery;

/// The defect the whole failure-reporting design starts from: a route name that
/// is well formed but names nothing on the gateway answers 404, and a 404 used
/// to retry on the schedule meant for a gateway that is down. Only four statuses
/// were terminal; every other refusal burned twenty attempts over seven days.
#[test]
fn a_refusal_outside_the_four_originally_recognized_statuses_now_stops_queued_retries() {
    for status in [400, 405, 410, 422] {
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
        delivery
            .retry(lease(100, 130), &|| Some(100), &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(
            store.delivery_health().unwrap().deadlettered_legs,
            1,
            "HTTP {status} must stop queued retries"
        );
    }
}

/// The other half of the same rule, and the reason it is a classifier rather
/// than a longer list: a status that says the fault may pass keeps every one of
/// its attempts. Getting this wrong drops pages that would have arrived.
#[test]
fn a_status_that_may_still_heal_keeps_its_retries() {
    for status in [408, 429, 500, 502, 503] {
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
        delivery
            .retry(lease(100, 130), &|| Some(100), &|_| {})
            .unwrap()
            .unwrap();
        let health = store.delivery_health().unwrap();
        assert_eq!(
            (health.deadlettered_legs, health.pending_legs),
            (0, 1),
            "HTTP {status} must stay queued"
        );
    }
}

/// A leg given up on records WHY in a spelling the constraint had to be widened
/// to admit, so a report can say "the gateway refused this" rather than "it ran
/// out of attempts", which is what the two existing reasons mean.
#[test]
fn a_refused_leg_records_permanent_rather_than_an_exhausted_counter() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    let input = remote_input();
    let destinations = destinations(404);
    let delivery = SubmissionDelivery {
        ledger: &store,
        decisions: &store,
        destinations: &destinations,
    };
    delivery
        .submit(&input, None, lease(10, 40), &|| Some(10), &|_| {})
        .unwrap();
    delivery
        .retry(lease(100, 130), &|| Some(100), &|_| {})
        .unwrap()
        .unwrap();
    let connection = rusqlite::Connection::open(path.join("pns.db")).unwrap();
    let recorded: (Option<String>, Option<u16>) = connection
        .query_row(
            "SELECT deadletter_reason,http_status FROM ledger_legs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(recorded, (Some("permanent".into()), Some(404)));
}

/// A retryable answer's code is kept on its own attempt, which is new: the leg's
/// `http_status` has always meant "the status it died of", so a 503 that will be
/// tried again left its code nowhere and a report could only say that something
/// failed. The old CHECK admitted four codes, so this row could not have been
/// written at all.
#[test]
fn a_retryable_answer_keeps_its_status_on_the_attempt_that_got_it() {
    let path = state();
    let store = SqliteStore::new(path.clone());
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
    let connection = rusqlite::Connection::open(path.join("pns.db")).unwrap();
    let attempt: Option<u16> = connection
        .query_row("SELECT http_status FROM ledger_attempts", [], |row| {
            row.get(0)
        })
        .unwrap();
    let leg: Option<u16> = connection
        .query_row("SELECT http_status FROM ledger_legs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(attempt, Some(503), "the attempt keeps the code it got");
    assert_eq!(leg, None, "the leg is not dead, so it died of nothing");
    // And the record still reads back as retryable rather than terminal: a
    // stored status no longer implies the leg was given up on.
    let record = store.inspect(&input.identity).unwrap().unwrap();
    assert!(matches!(
        record.attempts.last().unwrap().completion,
        LedgerCompletion::Retry { .. }
    ));
}
