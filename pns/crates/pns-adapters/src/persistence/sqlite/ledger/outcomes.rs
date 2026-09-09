use super::*;
use pns_domain::{
    Delivery,
    retry::{DeliveryOutcome, RetryBackoff},
};
use rusqlite::{Transaction, params};

pub(super) fn record(
    transaction: &Transaction<'_>,
    claim: &DeliveryClaim,
    delivery: &Delivery,
    at: u64,
    backoff: RetryBackoff,
) -> Result<Option<LedgerCompletion>, StoreError> {
    let completion = completion(claim, delivery, at, backoff);
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
         due = COALESCE(?2,due), deadlettered_at = ?6, http_status = ?7,
         deadletter_reason = COALESCE(?8, deadletter_reason)
         WHERE id = ?3 AND generation = ?4 AND token = ?5 AND acknowledged = 0 AND deadlettered_at IS NULL",
        params![outcome == 1,retry_at,claim.leg,claim.generation,claim.token,status.map(|_| at.to_be_bytes()),status,status.map(|_| "permanent")],
    )?;
    if changed == 0 {
        return Ok(None);
    }
    // The status THIS attempt got, whichever it was, which is what makes a
    // failure report possible: `status` above is the TERMINAL status and is set
    // only when the leg is being given up on, so a 503 that will be retried used
    // to leave no trace of its code anywhere. The leg keeps meaning "the status
    // it died of"; the attempt now means "the status this try got".
    let attempt_status = match delivery {
        Delivery::Rejected { status, .. } => Some(*status),
        _ => None,
    };
    let changed = transaction.execute(
        "UPDATE ledger_attempts SET finished = ?1, outcome = ?2, detail = ?3, retry_at = ?4, http_status = ?7
         WHERE leg = ?5 AND generation = ?6 AND finished IS NULL",
        params![at.to_be_bytes(),outcome,detail,retry_at,claim.leg,claim.generation,attempt_status],
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

/// INFALLIBLE, and no longer handed the transaction. It used to read
/// `randomblob(2)` for the retry jitter; the jitter is gone, so the retry time
/// is a pure function of the clock and the attempt count and this needs no
/// database at all.
fn completion(
    claim: &DeliveryClaim,
    delivery: &Delivery,
    at: u64,
    backoff: RetryBackoff,
) -> LedgerCompletion {
    let (outcome, detail) = match delivery {
        Delivery::Delivered(detail) => {
            return LedgerCompletion::Acknowledged {
                detail: detail.clone(),
            };
        }
        // Terminal, and this is the ONE place that decides it. A channel now
        // reports the status it got and says nothing about whether the gateway
        // will ever accept the page; the domain's classifier answers that, so
        // every destination stops on the same set of codes.
        Delivery::Rejected { status, detail }
            if claim.generation > 1 && DeliveryOutcome::Status(*status).class().is_permanent() =>
        {
            return LedgerCompletion::Rejected {
                status: *status,
                detail: detail.clone(),
            };
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
        backoff.retry_at(at, (claim.generation - 1) as u64)
    };
    LedgerCompletion::Retry {
        outcome,
        detail,
        retry_at,
    }
}
