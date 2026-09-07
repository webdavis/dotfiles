use super::*;
mod fixtures;
mod process;
use fixtures::{Attempt, Replay, holds};

#[test]
fn the_replay_use_case_keeps_its_hold_through_delivery_then_completes_it() {
    let replay = Replay::new("replay-completion-order", Attempt::Completed);
    replay.run();
    assert_eq!(replay.outcome.get(), Some(Attempt::Completed));
    assert!(
        holds(&replay.state).is_empty(),
        "a completed attempt releases the held batch"
    );
    assert!(!replay.state.join(crate::MISSED_NOTIFICATIONS).exists());
}

#[test]
fn a_completed_failed_replay_still_consumes_its_batch() {
    let replay = Replay::new("replay-completed-failure", Attempt::Failed);
    replay.run();
    assert_eq!(replay.outcome.get(), Some(Attempt::Failed));
    assert!(
        holds(&replay.state).is_empty(),
        "the existing completed-failure policy is consumption"
    );
}

#[test]
fn an_unwinding_replay_preserves_its_hold_and_a_live_racer_cannot_take_it() {
    let replay = Replay::new("replay-unwind-hold", Attempt::Interrupted);
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| replay.run()));
    assert!(interrupted.is_err());
    assert_eq!(replay.outcome.get(), None);
    let state = replay.state.clone();
    drop(replay);
    let held = holds(&state);
    assert_eq!(held.len(), 1, "drop must preserve an unfinished attempt");
    let bytes = std::fs::read(&held[0]).expect("the held bytes");
    let racer = FileReturnMoment::new(state.clone());
    assert!(
        racer
            .claim(Some(2_001), true)
            .expect("the next window")
            .waiting
            .is_empty()
    );
    racer.complete();
    assert_eq!(std::fs::read(&held[0]).expect("live owner's hold"), bytes);
}

#[test]
fn completion_removes_only_its_hold_and_preserves_a_new_journal() {
    let replay = Replay::new("replay-completion-new-journal", Attempt::Completed);
    let claim = replay.moment.claim(Some(2_000), true).expect("the claim");
    assert_eq!(claim.waiting[0].detail, "still owed");
    let journal = replay.state.join(crate::MISSED_NOTIFICATIONS);
    std::fs::write(&journal, "{\"detail\":\"new arrival\"}\n").expect("a later journal");
    replay.moment.complete();
    assert!(holds(&replay.state).is_empty());
    assert_eq!(
        std::fs::read(&journal).expect("the newer journal"),
        b"{\"detail\":\"new arrival\"}\n"
    );
}
