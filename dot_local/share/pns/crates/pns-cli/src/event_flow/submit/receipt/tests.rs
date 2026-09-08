use super::*;
use pns_application::LedgerLeg;
use pns_domain::routing::ReportMode;

#[test]
fn json_receipts_report_every_verdict_and_only_committed_work_is_accepted() {
    let replies = [
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
        assert_eq!(
            output.status,
            if sequence.is_some() {
                Status::Accepted
            } else {
                Status::Degraded
            }
        );
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
    let refused = result(Err(LedgerFailure::ConflictingSubmission));
    assert_eq!(refused.status, Status::Rejected);
    assert_eq!(refused.diagnostics, ["submission_conflict"]);
    assert!(refused.destinations.is_empty());
}
