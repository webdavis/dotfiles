use super::replay_handoff;
use pns_application::{
    LedgerFailure, LedgerSubmission, ReplayHandoff, SubmissionIdentity, SubmissionRecord, Submitted,
};

#[test]
fn only_a_persisted_or_existing_submission_transfers_the_replay_journal() {
    let attempted = |sequence| Submitted::Attempted {
        sequence,
        outcomes: Vec::new(),
    };
    assert_eq!(
        replay_handoff(Ok(attempted(None))),
        ReplayHandoff::Retained,
        "an unpersisted attempt cannot own the replay journal"
    );
    assert_eq!(
        replay_handoff(Err(LedgerFailure::ConflictingSubmission)),
        ReplayHandoff::Retained,
        "a refused handoff must preserve the replay journal"
    );
    assert_eq!(
        replay_handoff(Ok(attempted(Some(1)))),
        ReplayHandoff::Queued
    );
    let existing = Submitted::Existing(Box::new(SubmissionRecord {
        sequence: 1,
        submission: LedgerSubmission {
            producer_request: None,
            identity: SubmissionIdentity {
                producer: "pns-return".into(),
                request_id: "same-batch".into(),
            },
            event: Default::default(),
            legs: Vec::new(),
        },
        attempts: Vec::new(),
    }));
    assert_eq!(
        replay_handoff(Ok(existing)),
        ReplayHandoff::Queued,
        "the existing ledger submission already owns subsequent attempts"
    );
}
