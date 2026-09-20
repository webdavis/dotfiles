use super::*;
use pns_application::LedgerLeg;
use pns_domain::routing::ReportMode;

#[test]
fn json_receipts_report_every_verdict_and_only_committed_work_is_accepted() {
    let replies = [
        (
            Delivery::Rejected {
                status: 401,
                detail: "private detail".into(),
            },
            DeliveryOutcome::Failed,
        ),
        (
            Delivery::Delivered("private detail".into()),
            DeliveryOutcome::Delivered,
        ),
        (
            Delivery::Failed("private detail".into()),
            DeliveryOutcome::Failed,
        ),
        (
            Delivery::Unlaunched("private detail".into()),
            DeliveryOutcome::Unlaunched,
        ),
        (Delivery::Silent, DeliveryOutcome::Silent),
    ];
    for sequence in [None, Some(7)] {
        let output = result(Ok(Submitted::Attempted {
            sequence,
            outcomes: replies
                .iter()
                .enumerate()
                .map(|(index, (reply, _))| {
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
                .map(|(_, outcome)| *outcome)
                .collect::<Vec<_>>()
        );
        assert_eq!(output.decision_id, sequence.map(|id| id.to_string()));
        assert!(!output.encode().unwrap().contains("private detail"));
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
            (leg("macos-banner"), Delivery::Delivered("posted".into())),
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
