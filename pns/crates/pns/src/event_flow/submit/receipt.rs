use super::NotSubmitted;
use pns_application::{LedgerCompletion, LedgerFailure, Submitted, UnconfirmedDelivery};
use pns_domain::Delivery;
use pns_protocol::{DeliveryOutcome, DestinationOutcome, Name, ResultEnvelope, Status};

pub(super) fn result(submitted: Result<Submitted, NotSubmitted>) -> ResultEnvelope {
    // BAD INPUT LEAVES THE LEDGER OUT OF IT, and says which word was refused,
    // so a producer reading the reply learns the class it has to fix without
    // reading the machine's stderr.
    let submitted = match submitted {
        Err(NotSubmitted::UnknownDeliveryClass(class)) => {
            return ResultEnvelope {
                request_id: None,
                status: Status::Rejected,
                decision_id: None,
                interaction: None,
                destinations: Vec::new(),
                diagnostics: vec!["unknown_delivery_class".into(), class],
            };
        }
        Err(NotSubmitted::Ledger(error)) => Err(error),
        Ok(submitted) => Ok(submitted),
    };
    let (sequence, outcomes, failure) = match submitted {
        Ok(Submitted::Attempted { sequence, outcomes }) => (
            sequence,
            outcomes
                .into_iter()
                .map(|(leg, delivered)| (outcome(leg.destination, &delivered), leg.decorative))
                .collect(),
            None,
        ),
        Ok(Submitted::Existing(record)) => (
            Some(record.sequence),
            {
                let decorative: std::collections::HashSet<String> = record
                    .submission
                    .legs
                    .iter()
                    .filter(|leg| leg.decorative)
                    .map(|leg| leg.destination.clone())
                    .collect();
                record
                    .attempts
                    .into_iter()
                    .map(|attempt| {
                        let verdict = match attempt.completion {
                            LedgerCompletion::Rejected { .. } => DeliveryOutcome::Failed,
                            LedgerCompletion::Acknowledged { .. } => DeliveryOutcome::Delivered,
                            LedgerCompletion::Retry {
                                outcome: UnconfirmedDelivery::Failed,
                                ..
                            } => DeliveryOutcome::Failed,
                            LedgerCompletion::Retry {
                                outcome: UnconfirmedDelivery::Unlaunched,
                                ..
                            } => DeliveryOutcome::Unlaunched,
                            LedgerCompletion::Retry {
                                outcome: UnconfirmedDelivery::Unknown,
                                ..
                            } => DeliveryOutcome::Silent,
                        };
                        let is_decorative = decorative.contains(&attempt.destination);
                        (named(attempt.destination, verdict), is_decorative)
                    })
                    .collect()
            },
            None,
        ),
        Err(error) => (
            None,
            Vec::new(),
            Some(match error {
                LedgerFailure::ConflictingSubmission => "submission_conflict",
                LedgerFailure::InvalidPlan => "submission_plan_invalid",
                LedgerFailure::InvalidLease => "submission_lease_invalid",
                LedgerFailure::LostClaim => "submission_claim_lost",
                LedgerFailure::Unavailable(_) => "submission_unavailable",
            }),
        ),
    };
    let status = if failure.is_some() {
        Status::Rejected
    } else {
        delivered(&outcomes)
    };
    ResultEnvelope {
        request_id: None,
        status,
        decision_id: sequence.map(|id| id.to_string()),
        interaction: None,
        destinations: outcomes.into_iter().map(|(entry, _)| entry).collect(),
        diagnostics: vec![
            failure
                .unwrap_or(if sequence.is_some() {
                    "ledger_committed"
                } else {
                    "ledger_unavailable"
                })
                .into(),
        ],
    }
}

/// DELIVERY DECIDES THE STATUS, never the ledger: a committed row whose every
/// destination failed is `Undelivered` and says `ledger_committed` beside it.
/// A plan with no destination at all delivered everything it had.
///
/// SILENT IS AN ARRIVAL. It is the verdict of an executable channel that ran
/// and had nothing to say, which is the ordinary success on that path; only a
/// destination that FAILED or was never launched received nothing.
///
/// DECORATIVE LEGS DO NOT DECIDE IT, on the same terms as `event_flow::landed`:
/// a banner that could not spawn its notifier is a notification the operator
/// missed, not a page that is nowhere. Its verdict is still in the list.
fn delivered(outcomes: &[(DestinationOutcome, bool)]) -> Status {
    let durable: Vec<&DestinationOutcome> = outcomes
        .iter()
        .filter(|(_, decorative)| !decorative)
        .map(|(entry, _)| entry)
        .collect();
    let arrived = durable
        .iter()
        .filter(|entry| {
            matches!(
                entry.outcome,
                DeliveryOutcome::Delivered | DeliveryOutcome::Silent
            )
        })
        .count();
    if arrived == durable.len() {
        Status::Delivered
    } else if arrived == 0 {
        Status::Undelivered
    } else {
        Status::Partial
    }
}

fn outcome(destination: String, delivered: &Delivery) -> DestinationOutcome {
    named(
        destination,
        match delivered {
            Delivery::Delivered(_) => DeliveryOutcome::Delivered,
            Delivery::Failed(_) | Delivery::Rejected { .. } => DeliveryOutcome::Failed,
            Delivery::Silent => DeliveryOutcome::Silent,
            Delivery::Unlaunched(_) => DeliveryOutcome::Unlaunched,
        },
    )
}

fn named(destination: String, outcome: DeliveryOutcome) -> DestinationOutcome {
    DestinationOutcome {
        // Destination names come from the validated compiled registry.
        destination: Name::new(destination).expect("a registered destination name"),
        outcome,
        note: None,
    }
}

#[cfg(test)]
mod tests;
