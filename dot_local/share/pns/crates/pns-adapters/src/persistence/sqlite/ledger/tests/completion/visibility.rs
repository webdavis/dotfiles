use super::*;
#[test]
fn an_observer_sees_ledger_and_decision_completion_in_one_snapshot() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    let input = submission();
    let mut legs = created(&store, &input);
    begin(&store, &input.identity);
    let claim = legs.remove(0).claim;
    let mut observer = store.connect().unwrap();
    let (done, finished) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let store = SqliteStore::new(path);
        let result = store.record(&claim, &acknowledged(), 11);
        done.send(()).unwrap();
        result
    });
    let deadline = Instant::now() + Duration::from_millis(650);
    let mut inconsistent = false;
    loop {
        let (recorded, decision) = snapshot(&mut observer);
        inconsistent |= recorded != decision.ends_with(" legs=phone-primary:delivered\n");
        if finished.try_recv().is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "owned completion did not finish");
        std::thread::yield_now();
    }
    worker.join().unwrap().unwrap();
    let (recorded, decision) = snapshot(&mut observer);
    assert!(recorded && decision.ends_with(" legs=phone-primary:delivered\n"));
    assert!(
        !inconsistent,
        "a reader observed completion without its decision outcome"
    );
}
#[test]
fn a_suspended_worker_cannot_revise_after_a_competing_generation_finishes() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    let input = submission();
    let mut legs = created(&store, &input);
    begin(&store, &input.identity);
    let claim = legs.remove(0).claim;
    let (release, resumed) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        resumed.recv_timeout(Duration::from_millis(650)).unwrap();
        SqliteStore::new(path).record(&claim, &retry(40), 22)
    });
    let successor = store.claim_retry(lease(20, 30)).unwrap().unwrap();
    let result = store.record(&successor.claim, &acknowledged(), 21);
    release.send(()).unwrap();
    let stale = worker.join().unwrap();
    result.unwrap();
    assert_eq!(stale, Err(LedgerFailure::LostClaim));
    assert!(
        line(&store).ends_with(" legs=phone-primary:delivered\n"),
        "a stale completion must preserve the newer decision verdict"
    );
    let history = store.inspect(&input.identity).unwrap().unwrap();
    assert_eq!(history.attempts[2].completion, acknowledged());
}
