use super::*;
use pns_application::{
    LedgerLeg, LedgerSubmission, LegAttempt, SubmissionIdentity, SubmissionRecord,
};
use pns_domain::{Event, routing::ReportMode};

#[test]
fn json_receipts_report_every_verdict_and_only_committed_work_is_accepted() {
    // Each reply pairs its verdict with the note the receipt must carry: the
    // destination's own sentence about a leg it did not deliver, and nothing
    // for a leg that arrived.
    let replies = [
        (
            Delivery::Rejected {
                status: 401,
                detail: "the gateway said 401".into(),
            },
            DeliveryOutcome::Failed,
            Some("the gateway said 401"),
        ),
        (
            Delivery::Delivered("posted".into()),
            DeliveryOutcome::Delivered,
            None,
        ),
        (
            Delivery::Failed("the gateway refused".into()),
            DeliveryOutcome::Failed,
            Some("the gateway refused"),
        ),
        (
            Delivery::Unlaunched("no such channel".into()),
            DeliveryOutcome::Unlaunched,
            Some("no such channel"),
        ),
        (Delivery::Silent, DeliveryOutcome::Silent, None),
    ];
    for sequence in [None, Some(7)] {
        let output = result(Ok(Submitted::Attempted {
            sequence,
            outcomes: replies
                .iter()
                .enumerate()
                .map(|(index, (reply, ..))| {
                    (
                        LedgerLeg {
                            destination: format!("channel{index}"),
                            route: "priority".into(),
                            mode: ReportMode::ReportOutcome,
                            decorative: false,
                        },
                        reply.clone(),
                    )
                })
                .collect(),
        }));
        // ONE LEG DELIVERED AND FOUR DID NOT, whatever the ledger did: the
        // committed row is a diagnostic beside the status, never the status.
        assert_eq!(output.status, Status::Partial);
        assert_eq!(
            output
                .destinations
                .iter()
                .map(|entry| entry.outcome)
                .collect::<Vec<_>>(),
            replies
                .iter()
                .map(|(_, outcome, _)| *outcome)
                .collect::<Vec<_>>()
        );
        // THE DESTINATION'S OWN SENTENCE RIDES BACK, on the route the leg was
        // submitted on.
        assert_eq!(
            output
                .destinations
                .iter()
                .map(|entry| entry.note.as_deref())
                .collect::<Vec<_>>(),
            replies.iter().map(|(_, _, note)| *note).collect::<Vec<_>>()
        );
        assert!(
            output
                .destinations
                .iter()
                .all(|entry| entry.route.as_ref().map(Name::as_str) == Some("priority"))
        );
        assert_eq!(output.ledger_sequence, sequence.map(|id| id.to_string()));
        assert_eq!(
            output
                .diagnostics
                .iter()
                .any(|code| code == "ledger_committed"),
            sequence.is_some()
        );
    }
    let refused = result(Err(NotSubmitted::Ledger(
        LedgerFailure::ConflictingSubmission,
    )));
    assert_eq!(refused.status, Status::Rejected);
    assert_eq!(refused.diagnostics, ["submission_conflict"]);
    assert!(refused.destinations.is_empty());

    // THE CLASS IS NAMED IN THE REPLY, so a producer reading stdout alone
    // learns which word it has to fix rather than only that something was
    // refused.
    let undefined = result(Err(NotSubmitted::UnknownDeliveryClass("security".into())));
    assert_eq!(undefined.status, Status::Rejected);
    assert_eq!(
        undefined.diagnostics,
        ["unknown_delivery_class", "security"]
    );
    assert!(undefined.destinations.is_empty());
}

/// THE WHOLE POINT OF THE SLICE: a row that committed and a page that reached
/// nobody is not a success, and the receipt says both facts at once.
#[test]
fn a_committed_row_whose_every_destination_failed_is_undelivered() {
    let output = result(Ok(Submitted::Attempted {
        sequence: Some(7),
        outcomes: vec![(
            leg("hermes"),
            Delivery::Failed("the gateway refused".into()),
        )],
    }));
    assert_eq!(output.status, Status::Undelivered);
    assert_eq!(output.diagnostics, ["ledger_committed"]);
}

/// A page every destination took, which is the only shape that earns exit 0.
#[test]
fn a_page_every_destination_took_is_delivered() {
    let output = result(Ok(Submitted::Attempted {
        sequence: Some(7),
        outcomes: vec![
            (leg("hermes"), Delivery::Delivered("posted".into())),
            (leg("banner"), Delivery::Delivered("posted".into())),
        ],
    }));
    assert_eq!(output.status, Status::Delivered);
}

/// A plan with no destination at all delivered everything it had. Answering
/// `undelivered` here would fail a caller that narrowed the event itself.
#[test]
fn a_plan_with_no_destination_delivered_everything_it_had() {
    let output = result(Ok(Submitted::Attempted {
        sequence: Some(7),
        outcomes: Vec::new(),
    }));
    assert_eq!(output.status, Status::Delivered);
}

fn leg(destination: &str) -> LedgerLeg {
    LedgerLeg {
        destination: destination.into(),
        route: "priority".into(),
        mode: ReportMode::ReportOutcome,
        decorative: false,
    }
}

/// A REPLAYED submission reports what the ledger stored: the destination's
/// own sentence, the route the leg was submitted on, and the moment the next
/// attempt is due. The ledger stores a live Silent leg as an unresolved
/// retry with no detail, so replaying it back reports the same quiet
/// arrival the first attempt did rather than the missing-answer word.
#[test]
fn a_replayed_leg_carries_its_note_its_route_and_the_time_it_is_retried() {
    let completions = [
        (
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Failed,
                detail: "the gateway refused".into(),
                retry_at: 1_758_153_600,
            },
            DeliveryOutcome::Failed,
            Some("the gateway refused"),
            Some(1_758_153_600),
        ),
        (
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unknown,
                detail: String::new(),
                retry_at: 1_758_153_600,
            },
            DeliveryOutcome::Silent,
            None,
            Some(1_758_153_600),
        ),
        (
            LedgerCompletion::Rejected {
                status: 401,
                detail: "the gateway refused".into(),
            },
            DeliveryOutcome::Failed,
            Some("the gateway refused"),
            None,
        ),
    ];
    for (completion, outcome, note, retry_at) in completions {
        let output = result(Ok(Submitted::Existing(Box::new(record(completion)))));
        let entry = &output.destinations[0];
        assert_eq!(entry.outcome, outcome);
        assert_eq!(entry.note.as_deref(), note);
        assert_eq!(entry.route.as_ref().map(Name::as_str), Some("priority"));
        assert_eq!(entry.retry_at, retry_at);
    }
}

/// A retried leg with no stored detail (the ledger's Silent shape, and a
/// dead-lettered `Rejected` the destination gave no reason for) reports no
/// note at all: an empty string is not a sentence, so it is omitted rather
/// than sent as `note: ""`.
#[test]
fn a_replayed_leg_with_no_stored_detail_omits_the_note_rather_than_sending_an_empty_one() {
    for completion in [
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Unknown,
            detail: String::new(),
            retry_at: 1_758_153_600,
        },
        LedgerCompletion::Rejected {
            status: 401,
            detail: String::new(),
        },
    ] {
        let output = result(Ok(Submitted::Existing(Box::new(record(completion)))));
        assert_eq!(output.destinations[0].note, None);
    }
}

/// THE EVENT'S OWN TEXT NEVER COMES BACK, whatever else the receipt carries:
/// `note` is the destination's sentence, and a producer that wants its own
/// detail back already has it.
#[test]
fn the_events_own_text_never_appears_anywhere_in_the_receipt() {
    let mut stored = record(LedgerCompletion::Acknowledged {
        detail: "accepted".into(),
    });
    stored.submission.event.detail = "private detail".into();
    stored.submission.event.message = "private detail".into();
    let output = result(Ok(Submitted::Existing(Box::new(stored))));
    assert!(!output.encode().unwrap().contains("private detail"));
}

fn record(completion: LedgerCompletion) -> SubmissionRecord {
    SubmissionRecord {
        sequence: 7,
        submission: LedgerSubmission {
            identity: SubmissionIdentity {
                producer: "nvim".into(),
                request_id: "r-1".into(),
            },
            producer_request: None,
            event: Event::default(),
            legs: vec![leg("hermes")],
        },
        attempts: vec![LegAttempt {
            destination: "hermes".into(),
            generation: 1,
            at: 1_758_153_000,
            completion,
        }],
    }
}
