use super::{DeliveryClaim, LedgerCompletion, SubmissionIdentity, UnconfirmedDelivery};
use crate::persistence::sqlite::{StoreError, decisions};
use pns_domain::Delivery;
use rusqlite::Transaction;

// Called only after this transaction has accepted the opaque claim generation.
// The same commit publishes the attempt and its printable decision verdict.
pub(super) fn revise_decision(
    transaction: &Transaction<'_>,
    claim: &DeliveryClaim,
    completion: &LedgerCompletion,
) -> Result<(), StoreError> {
    let (identity, destination, decorative) = transaction.query_row(
        "SELECT e.producer, e.request_id, l.destination, l.decorative FROM ledger_legs l
         JOIN ledger_events e ON e.seq = l.event WHERE l.id = ?1",
        [claim.leg],
        |row| {
            Ok((
                SubmissionIdentity {
                    producer: row.get(0)?,
                    request_id: row.get(1)?,
                },
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
            ))
        },
    )?;
    let delivery = match completion {
        LedgerCompletion::Rejected { status, detail } => Delivery::Rejected {
            status: *status,
            detail: detail.clone(),
        },
        LedgerCompletion::Acknowledged { detail } => Delivery::Delivered(detail.clone()),
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Failed,
            detail,
            ..
        } => Delivery::Failed(detail.clone()),
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Unlaunched,
            detail,
            ..
        } => Delivery::Unlaunched(detail.clone()),
        LedgerCompletion::Retry {
            outcome: UnconfirmedDelivery::Unknown,
            ..
        } => Delivery::Silent,
    };
    // Pruning removes only the diagnostic history, not dispatch ownership.
    decisions::revise(transaction, &identity, &destination, &delivery)?;
    if decorative && matches!(completion, LedgerCompletion::Acknowledged { .. }) {
        super::super::journal::acknowledge(transaction, &identity)?;
    }
    Ok(())
}
