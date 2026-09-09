use super::*;
use crate::state_fixtures::scratch;

#[test]
fn an_expired_claim_is_removed_without_consuming_a_later_prompt() {
    let state = scratch("turn-claim-new");
    let path = state.join("session-one.start");
    let claim = state.join("session-one.start.sweep.7");
    std::fs::write(&claim, "1").unwrap();
    std::fs::write(&path, "1000000").unwrap();
    finish_claim(&path, &claim, 1_000_000);
    assert!(!claim.exists());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "1000000");
}

#[test]
fn a_claim_that_became_fresh_is_restored_without_replacing_an_arrival() {
    let state = scratch("turn-claim-fresh");
    let path = state.join("session-one.start");
    let claim = state.join("session-one.start.sweep.7");
    std::fs::write(&claim, "999999").unwrap();
    finish_claim(&path, &claim, 1_000_000);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "999999");
    assert!(!claim.exists());
    std::fs::write(&claim, "999999").unwrap();
    std::fs::write(&path, "1000000").unwrap();
    finish_claim(&path, &claim, 1_000_000);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "1000000");
    assert_eq!(std::fs::read_to_string(claim).unwrap(), "999999");
}

#[test]
fn a_sweep_claim_already_owned_by_another_invocation_is_untouched() {
    let state = scratch("turn-claim-owned");
    let path = state.join("session-one.start");
    let claim = path.with_extension(format!("start.sweep.{}", std::process::id()));
    std::fs::write(&path, "1").unwrap();
    std::fs::write(&claim, "another owner").unwrap();
    sweep_turn_markers(&state, Some(1_000_000));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "1");
    assert_eq!(std::fs::read_to_string(claim).unwrap(), "another owner");
}

#[test]
fn the_sweep_preserves_links_and_directories_and_restores_torn_claims() {
    let state = scratch("turn-claim-shape");
    let path = state.join("session-one.start");
    let target = state.join("outside");
    std::fs::write(&target, "1").unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let directory = state.join("session-directory.start");
    std::fs::create_dir(&directory).unwrap();
    sweep_turn_markers(&state, Some(1_000_000));
    assert_eq!(std::fs::read_link(path).unwrap(), target);
    assert!(directory.is_dir());
    let path = state.join("session-torn.start");
    let claim = state.join("session-torn.start.sweep.7");
    std::fs::write(&claim, "torn").unwrap();
    finish_claim(&path, &claim, 1_000_000);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "torn");
}
