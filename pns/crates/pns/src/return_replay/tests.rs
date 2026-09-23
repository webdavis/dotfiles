use super::{recap_destination, replay_handoff};
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

#[test]
fn the_card_names_where_each_log_transport_posts_the_recap() {
    // HERMES POSTS ON THE DEFAULT ROUTE AND IGNORES THE PROJECT; the Discord
    // bot posts to the channel its map resolves for the recap's own project
    // on the empty route.
    let channels: pns_domain::channel_map::ChannelMap =
        [("default", "1"), ("dotfiles", "2"), ("logbook", "3")]
            .into_iter()
            .map(|(key, channel)| (key.to_string(), channel.to_string()))
            .collect();
    let dotfiles = || "dotfiles".to_string();
    assert_eq!(
        recap_destination(Some("hermes"), &channels, "logbook", dotfiles),
        Some("logbook".to_string())
    );
    assert_eq!(
        recap_destination(Some("discord"), &channels, "logbook", dotfiles),
        Some("dotfiles".to_string())
    );
    assert_eq!(
        recap_destination(Some("discord"), &channels, "logbook", String::new),
        Some("logbook".to_string()),
        "a recap composed outside a repository lands on the default route's key"
    );
    assert_eq!(
        recap_destination(None, &channels, "logbook", dotfiles),
        None
    );
}
