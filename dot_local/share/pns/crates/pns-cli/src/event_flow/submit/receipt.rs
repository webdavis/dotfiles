use pns_application::{LedgerCompletion, LedgerFailure, Submitted, UnconfirmedDelivery};
use pns_domain::Delivery;
use pns_protocol::{DeliveryOutcome, DestinationOutcome, Name, ResultEnvelope, Status};

pub(super) fn result(submitted: Result<Submitted, LedgerFailure>) -> ResultEnvelope {
    let (sequence, outcomes, failure) = match submitted {
        Ok(Submitted::Attempted { sequence, outcomes }) => (
            sequence,
            outcomes
                .into_iter()
                .map(|(leg, delivered)| outcome(leg.destination, &delivered))
                .collect(),
            None,
        ),
        Ok(Submitted::Existing(record)) => (
            Some(record.sequence),
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
                    named(attempt.destination, verdict)
                })
                .collect(),
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
    } else if sequence.is_some() {
        Status::Accepted
    } else {
        Status::Degraded
    };
    ResultEnvelope {
        request_id: None,
        status,
        decision_id: sequence.map(|id| id.to_string()),
        interaction: None,
        destinations: outcomes,
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
