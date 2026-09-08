use super::*;
#[test]
fn a_malformed_decision_rolls_back_completion_and_keeps_the_owned_attempt_unfinished() {
    let store = SqliteStore::new(state());
    let input = submission();
    let legs = created(&store, &input);
    begin(&store, &input.identity);
    store
        .transaction(|tx| {
            tx.execute(
                "UPDATE decisions SET line = 'original legs=phone-primary:invented\n'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let before = store.inspect(&input.identity).unwrap().unwrap();
    let decision = line(&store);
    assert!(
        matches!(
            store.record(&legs[0].claim, &acknowledged(), 11),
            Err(LedgerFailure::Unavailable(_))
        ),
        "malformed decision must roll back ledger completion"
    );
    assert_eq!(store.inspect(&input.identity).unwrap().unwrap(), before);
    assert_eq!(line(&store), decision);
    assert!(store.claim_retry(lease(19, 30)).unwrap().is_none());
    assert!(
        std::fs::read_to_string(&store.log)
            .unwrap()
            .contains("delivery ledger")
    );
}
#[test]
fn a_decision_write_refusal_rolls_back_completion_without_losing_claim_ownership() {
    let store = SqliteStore::new(state());
    let input = submission();
    let legs = created(&store, &input);
    begin(&store, &input.identity);
    store
        .transaction(|tx| {
            tx.execute_batch(
                "CREATE TRIGGER refuse_delivered BEFORE UPDATE ON decisions
      WHEN NEW.line LIKE '%:delivered%' BEGIN SELECT RAISE(ABORT, 'owned fixture refusal'); END;",
            )?;
            Ok(())
        })
        .unwrap();
    let before = store.inspect(&input.identity).unwrap().unwrap();
    let decision = line(&store);
    assert!(
        store.record(&legs[0].claim, &acknowledged(), 11).is_err(),
        "decision publication refusal must roll back ledger completion"
    );
    assert_eq!(store.inspect(&input.identity).unwrap().unwrap(), before);
    assert_eq!(line(&store), decision);
    store.record(&legs[0].claim, &retry(30), 12).unwrap();
    assert!(line(&store).ends_with(" legs=phone-primary:failed\n"));
    assert_eq!(
        store.inspect(&input.identity).unwrap().unwrap().attempts[0].completion,
        retry(30)
    );
}
