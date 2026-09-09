use super::*;
use pns_domain::{Delivery, retry::RetryBackoff};
use rusqlite::{Transaction, params};

pub(super) fn record(
    transaction: &Transaction<'_>,
    claim: &DeliveryClaim,
    delivery: &Delivery,
    at: u64,
    backoff: RetryBackoff,
) -> Result<Option<LedgerCompletion>, StoreError> {
    let completion = completion(transaction, claim, delivery, at, backoff)?;
    let (outcome, detail, retry_at, status) = match &completion {
        LedgerCompletion::Acknowledged { detail } => (1, detail, None, None),
        LedgerCompletion::Rejected { status, detail } => (2, detail, None, Some(*status)),
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
            None,
        ),
    };
    // Generation and token arbitrate completion. A late result may settle its
    // claim until a successor takes ownership; terminal failure cannot take it back.
    let changed = transaction.execute(
        "UPDATE ledger_legs SET acknowledged = ?1, owner = NULL, token = NULL, lease_until = NULL,
         due = COALESCE(?2,due), deadlettered_at = ?6, http_status = ?7
         WHERE id = ?3 AND generation = ?4 AND token = ?5 AND acknowledged = 0 AND deadlettered_at IS NULL",
        params![outcome == 1,retry_at,claim.leg,claim.generation,claim.token,status.map(|_| at.to_be_bytes()),status],
    )?;
    if changed == 0 {
        return Ok(None);
    }
    let changed = transaction.execute(
        "UPDATE ledger_attempts SET finished = ?1, outcome = ?2, detail = ?3, retry_at = ?4, http_status = ?7
         WHERE leg = ?5 AND generation = ?6 AND finished IS NULL",
        params![at.to_be_bytes(),outcome,detail,retry_at,claim.leg,claim.generation,status],
    )?;
    if changed != 1 {
        return Err(StoreError::InvalidState(
            "missing unfinished ledger attempt".into(),
        ));
    }
    if status.is_some() {
        super::health::raise_alarm(transaction)?;
    }
    Ok(Some(completion))
}

fn completion(
    transaction: &Transaction<'_>,
    claim: &DeliveryClaim,
    delivery: &Delivery,
    at: u64,
    backoff: RetryBackoff,
) -> Result<LedgerCompletion, StoreError> {
    let (outcome, detail) = match delivery {
        Delivery::Delivered(detail) => {
            return Ok(LedgerCompletion::Acknowledged {
                detail: detail.clone(),
            });
        }
        Delivery::Rejected { status, detail } if claim.generation > 1 => {
            return Ok(LedgerCompletion::Rejected {
                status: *status,
                detail: detail.clone(),
            });
        }
        Delivery::Failed(detail) | Delivery::Rejected { detail, .. } => {
            (UnconfirmedDelivery::Failed, detail.clone())
        }
        Delivery::Unlaunched(detail) => (UnconfirmedDelivery::Unlaunched, detail.clone()),
        Delivery::Silent => (UnconfirmedDelivery::Unknown, String::new()),
    };
    // Initial sends remain immediately eligible. Only queued claims consume the
    // linear schedule, including claims interrupted before their outcome was saved.
    let retry_at = if claim.generation == 1 {
        at
    } else {
        let sample = if backoff.random_secs == 0 {
            0
        } else {
            let bytes: [u8; 2] =
                transaction.query_row("SELECT randomblob(2)", [], |row| row.get(0))?;
            u16::from_be_bytes(bytes)
        };
        backoff.retry_at(at, (claim.generation - 1) as u64, sample)
    };
    Ok(LedgerCompletion::Retry {
        outcome,
        detail,
        retry_at,
    })
}
