use super::*;
use crate::state_fixtures::scratch;

#[test]
fn two_record_claimers_have_one_owner_even_without_the_fire_lock() {
    let state = scratch("nag-claim-race");
    let record = state.join("one.pending");
    std::fs::write(&record, "one approval").unwrap();
    let gate = std::sync::Barrier::new(3);
    let claimed = std::thread::scope(|scope| {
        let racers: Vec<_> = [11, 12]
            .into_iter()
            .map(|owner| {
                let record = &record;
                let gate = &gate;
                scope.spawn(move || {
                    gate.wait();
                    claim_record_as(record, owner)
                })
            })
            .collect();
        gate.wait();
        racers
            .into_iter()
            .filter_map(|racer| racer.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        claimed.len(),
        1,
        "only one independent record owner can consume an approval"
    );
    assert_eq!(std::fs::read(&claimed[0]).unwrap(), b"one approval");
    assert!(!record.exists());
}

#[test]
fn a_record_claim_never_overwrites_a_batch_that_owner_already_holds() {
    let state = scratch("nag-claim-existing");
    let record = state.join("one.pending");
    let claim = super::super::claim_path(&record, 11);
    std::fs::write(&record, "new approval").unwrap();
    std::fs::write(&claim, "prior approval").unwrap();
    assert!(claim_record_as(&record, 11).is_none());
    assert_eq!(std::fs::read(record).unwrap(), b"new approval");
    assert_eq!(std::fs::read(claim).unwrap(), b"prior approval");
}

#[test]
fn the_fire_claim_expires_only_after_its_full_minute() {
    let state = scratch("nag-fire-age-boundary");
    let lock = claim_fire(&state, 0).expect("first owner");
    let at = std::fs::metadata(&lock)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(claim_fire(&state, at.saturating_sub(1)).is_none());
    assert!(claim_fire(&state, at + 59).is_none());
    assert!(claim_fire(&state, at + 60).is_none());
    assert_eq!(claim_fire(&state, at + 61), Some(lock));
}
