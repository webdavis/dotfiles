use super::*;
use rusqlite::{Transaction, params};

pub(super) fn record(
    transaction: &Transaction<'_>,
    claim: &DeliveryClaim,
    completion: &LedgerCompletion,
    at: u64,
) -> Result<bool, StoreError> {
    let (outcome, detail, retry_at) = match completion {
        LedgerCompletion::Acknowledged { detail } => (1, detail, None),
        LedgerCompletion::Retry {
            outcome,
            detail,
            retry_at,
        } => (
            match outcome {
                UnconfirmedDelivery::Unknown => 0,
                UnconfirmedDelivery::Failed => 2,
                UnconfirmedDelivery::Unlaunched => 3,
            },
            detail,
            Some(retry_at.to_be_bytes()),
        ),
    };
    // The generation and opaque token arbitrate completion, never elapsed time.
    // A late result may settle its own claim until a retry actually takes it.
    let changed = transaction.execute(
        "UPDATE ledger_legs SET acknowledged = ?1, owner = NULL, token = NULL, lease_until = NULL,
         due = COALESCE(?2,due) WHERE id = ?3 AND generation = ?4 AND token = ?5 AND acknowledged = 0",
        params![outcome == 1,retry_at,claim.leg,claim.generation,claim.token],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    let changed = transaction.execute(
        "UPDATE ledger_attempts SET finished = ?1, outcome = ?2, detail = ?3, retry_at = ?4
         WHERE leg = ?5 AND generation = ?6 AND finished IS NULL",
        params![
            at.to_be_bytes(),
            outcome,
            detail,
            retry_at,
            claim.leg,
            claim.generation
        ],
    )?;
    if changed != 1 {
        return Err(StoreError::InvalidState(
            "missing unfinished ledger attempt".into(),
        ));
    }
    Ok(true)
}
