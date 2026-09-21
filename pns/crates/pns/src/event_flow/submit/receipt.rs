use super::NotSubmitted;
use pns_application::{
    LedgerCompletion, LedgerFailure, SubmissionRecord, Submitted, UnconfirmedDelivery,
};
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
                ledger_sequence: None,
                destinations: Vec::new(),
                diagnostics: vec!["unknown_delivery_class".into(), class],
                ignored_fields: Vec::new(),
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
                .filter_map(|(leg, delivered)| {
                    Some((
                        outcome(leg.destination, leg.route, &delivered)?,
                        leg.decorative,
                    ))
                })
                .collect(),
            None,
        ),
        Ok(Submitted::Existing(record)) => (Some(record.sequence), existing_outcomes(record), None),
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
        ledger_sequence: sequence.map(|id| id.to_string()),
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
        ignored_fields: Vec::new(),
    }
}

/// The verdict each leg of a REPLAYED submission gets, reconstructed from its
/// stored completion rather than a fresh `Delivery`.
fn existing_outcomes(record: Box<SubmissionRecord>) -> Vec<(DestinationOutcome, bool)> {
    let legs: std::collections::HashMap<String, (String, bool)> = record
        .submission
        .legs
        .iter()
        .map(|leg| (leg.destination.clone(), (leg.route.clone(), leg.decorative)))
        .collect();
    record
        .attempts
        .into_iter()
        .filter_map(|attempt| {
            let (verdict, note, retry_at) = match attempt.completion {
                LedgerCompletion::Rejected { detail, .. } => (
                    DeliveryOutcome::Failed,
                    Some(detail).filter(|detail| !detail.is_empty()),
                    None,
                ),
                LedgerCompletion::Acknowledged { .. } => (DeliveryOutcome::Delivered, None, None),
                LedgerCompletion::Retry {
                    outcome,
                    detail,
                    retry_at,
                } => (
                    match outcome {
                        UnconfirmedDelivery::Failed => DeliveryOutcome::Failed,
                        UnconfirmedDelivery::Unlaunched => DeliveryOutcome::Unlaunched,
                        // The ledger persists a live Silent as this retry outcome
                        // (see sqlite/ledger/outcomes.rs), so replaying it back as
                        // Silent here reports the same arrival the first attempt
                        // did; the ledger keeps retrying it in the background
                        // regardless.
                        UnconfirmedDelivery::Unknown => DeliveryOutcome::Silent,
                    },
                    Some(detail).filter(|detail| !detail.is_empty()),
                    Some(retry_at),
                ),
            };
            let (route, decorative) = legs.get(&attempt.destination).cloned().unwrap_or_default();
            Some((
                named(attempt.destination, route, verdict, note, retry_at)?,
                decorative,
            ))
        })
        .collect()
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

/// The sentence a destination offered about a leg it did not deliver. A
/// delivery's own text is the event coming back, so only the three verdicts
/// that explain a shortfall carry one.
fn outcome(destination: String, route: String, delivered: &Delivery) -> Option<DestinationOutcome> {
    let (verdict, note) = match delivered {
        Delivery::Delivered(_) => (DeliveryOutcome::Delivered, None),
        Delivery::Failed(note) => (DeliveryOutcome::Failed, Some(note.clone())),
        Delivery::Rejected { detail, .. } => (DeliveryOutcome::Failed, Some(detail.clone())),
        Delivery::Silent => (DeliveryOutcome::Silent, None),
        Delivery::Unlaunched(note) => (DeliveryOutcome::Unlaunched, Some(note.clone())),
    };
    named(destination, route, verdict, note, None)
}

/// One leg's verdict, or `None` for a destination name the envelope cannot
/// carry.
///
/// THE REGISTRY REFUSES SUCH A NAME AT REGISTRATION, so a planned leg always
/// yields a verdict here; this returns the absence rather than panicking on
/// the delivery path, where the crash would take the whole submission with it.
/// A route the envelope cannot carry is reported as no route instead, because
/// the route is advisory and the verdict beside it is what says whether the
/// event arrived.
fn named(
    destination: String,
    route: String,
    outcome: DeliveryOutcome,
    note: Option<String>,
    retry_at: Option<u64>,
) -> Option<DestinationOutcome> {
    Some(DestinationOutcome {
        name: Name::new(destination).ok()?,
        outcome,
        // A leg submitted to a destination's own default carries no route.
        route: if route.is_empty() {
            None
        } else {
            Name::new(route).ok()
        },
        note,
        retry_at,
    })
}

#[cfg(test)]
mod tests;
