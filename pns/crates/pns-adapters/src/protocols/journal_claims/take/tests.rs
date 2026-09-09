use super::*;
use crate::state_fixtures::scratch;

#[test]
fn a_read_claim_stays_on_disk_for_its_live_owner_until_completion() {
    let state = scratch("replay-hold-until-completion");
    let claim = state.join("missed-notifications.claim.fixture");
    std::fs::write(&claim, "{\"detail\":\"still owed\"}\n").expect("a claimed entry");
    assert!(matches!(take_claim(&claim), Claimed::Taken(_)));
    let held: Vec<_> = std::fs::read_dir(&state)
        .expect("the state directory")
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("missed-notifications.held.")
        })
        .collect();
    assert_eq!(
        held.len(),
        1,
        "reading a claim must not erase it before a replay outcome exists"
    );
    assert_eq!(
        std::fs::read(held[0].path()).expect("the hold"),
        b"{\"detail\":\"still owed\"}\n"
    );
}

#[test]
fn an_existing_claim_for_this_process_preserves_both_waiting_batches() {
    let state = crate::state_fixtures::scratch("journal-pid-claim");
    let journal = state.join(crate::MISSED_NOTIFICATIONS);
    let claim = journal.with_extension(format!("claim.{}", std::process::id()));
    std::fs::write(&journal, "new waiting batch").unwrap();
    std::fs::write(&claim, "earlier undelivered batch").unwrap();
    assert!(matches!(
        claim_by_rename(&journal),
        Claimed::LeftForAdoption
    ));
    assert_eq!(std::fs::read(journal).unwrap(), b"new waiting batch");
    assert_eq!(std::fs::read(claim).unwrap(), b"earlier undelivered batch");
}
