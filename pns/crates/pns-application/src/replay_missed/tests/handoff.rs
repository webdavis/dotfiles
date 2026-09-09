use super::*;

#[test]
fn a_replay_not_owned_by_the_ledger_preserves_its_journal() {
    let mut recorder = Recorder::new(Some(claim_of(None, vec![entry(1)])));
    recorder.handoff = crate::ReplayHandoff::Retained;
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), false);
    assert_eq!(recorder.delivered.borrow().len(), 1);
    assert_eq!(
        recorder.completed.get(),
        0,
        "unpersisted replay must retain its claim"
    );
}

#[test]
fn an_adopted_queued_replay_completes_without_publishing_or_dispatching_again() {
    let mut claim = claim_of(Some(1_000), vec![entry(1_500)]);
    claim.replay.as_mut().unwrap().state = crate::ReplayState::Queued;
    let mut recorder = Recorder::new(Some(claim));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert!(
        recorder.delivered.borrow().is_empty(),
        "ledger owns every later attempt"
    );
    assert!(
        recorder.publications.borrow().is_empty(),
        "adoption must not republish a digest"
    );
    assert_eq!(recorder.completed.get(), 1);
}

#[test]
fn a_replay_keeps_the_original_batch_identity_and_window_after_adoption() {
    let recorder = Recorder::new(Some(claim_of(Some(1_000), vec![entry(1_500)])));
    let mut decision = returning(vec![leg(true)]);
    decision.inputs.now_secs = Some(3_000);
    ports(&recorder).run(&decision, policy(), true);
    assert!(recorder.steps().contains(&"entries(1000,2000)".into()));
    assert_eq!(recorder.identities.borrow()[0].request_id, "original-batch");
    assert_eq!(recorder.completed.get(), 1);
}
