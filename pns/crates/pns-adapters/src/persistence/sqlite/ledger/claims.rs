use super::*;
use rusqlite::{Transaction, params};

pub(super) fn started(
    transaction: &Transaction<'_>,
    leg: i64,
    lease: LeaseWindow,
) -> Result<DeliveryClaim, StoreError> {
    let claim = transaction.query_row(
        "SELECT generation, token FROM ledger_legs WHERE id = ?1",
        [leg],
        |row| {
            Ok(DeliveryClaim {
                leg,
                generation: row.get(0)?,
                token: row.get(1)?,
            })
        },
    )?;
    transaction.execute(
        "INSERT INTO ledger_attempts(leg,generation,started,outcome,detail,retry_at) VALUES (?1,?2,?3,0,'',?4)",
        params![leg,claim.generation,lease.now.to_be_bytes(),lease.until.to_be_bytes()],
    )?;
    Ok(claim)
}

pub(super) fn next(
    transaction: &Transaction<'_>,
    lease: LeaseWindow,
    limits: pns_domain::retry::RetryLimits,
) -> Result<Option<RetryDelivery<DeliveryClaim>>, StoreError> {
    let mut query = transaction.prepare(
        "SELECT id,event,owner,lease_until,due,generation,
          (SELECT started FROM ledger_attempts WHERE leg = ledger_legs.id AND generation = 1) FROM ledger_legs
         WHERE acknowledged = 0 AND deadlettered_at IS NULL ORDER BY event,position",
    )?;
    let mut rows = query.query([])?;
    let mut selected = None;
    let mut exhausted = Vec::new();
    while let Some(row) = rows.next()? {
        let owner: Option<u32> = row.get(2)?;
        let eligible = match owner {
            Some(owner) => {
                u64::from_be_bytes(row.get(3)?) <= lease.now
                    || crate::marker_files::owner_is_gone(&owner.to_string())
            }
            None => u64::from_be_bytes(row.get(4)?) <= lease.now,
        };
        if eligible {
            let generation: u64 = row.get(5)?;
            let created = u64::from_be_bytes(row.get(6)?);
            // Generation 1 is the initial send. Each later claim consumes one
            // retry even when its process dies before recording an outcome.
            if let Some(reason) = limits.exhausted(generation - 1, created, lease.now) {
                exhausted.push((row.get::<_, i64>(0)?, reason));
                continue;
            }
            if selected.is_some() {
                continue;
            }
            selected = Some((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(5)?,
            ));
        }
    }
    drop(rows);
    drop(query);
    for (leg, reason) in &exhausted {
        // This sweep reads the two COUNTER limits, so it can only ever produce
        // Attempts or Age. Permanent is decided at the moment of the failure by
        // `RetryLimits::verdict`, and the write path for it arrives with the
        // schema change that widens `deadletter_reason`'s CHECK constraint to
        // admit the spelling. Until then this arm is unreachable, and writing
        // it would be refused by the constraint rather than stored wrongly.
        let reason = match reason {
            pns_domain::retry::DeadletterReason::Attempts => "attempts",
            pns_domain::retry::DeadletterReason::Age => "age",
            pns_domain::retry::DeadletterReason::Permanent => "permanent",
        };
        transaction.execute(
            "UPDATE ledger_legs SET deadlettered_at = ?1, deadletter_reason = ?2,
            owner = NULL, token = NULL, lease_until = NULL WHERE id = ?3",
            params![lease.now.to_be_bytes(), reason, leg],
        )?;
    }
    if !exhausted.is_empty() {
        super::health::raise_alarm(transaction)?;
    }
    let Some((leg, event, generation)) = selected else {
        return Ok(None);
    };
    let generation = generation
        .checked_add(1)
        .ok_or_else(|| StoreError::InvalidState("ledger generation exhausted".into()))?;
    let record = read::record(transaction, event)?;
    let route = transaction.query_row(
        "SELECT destination,route,mode,decorative FROM ledger_legs WHERE id = ?1",
        [leg],
        read::leg,
    )?;
    transaction.execute(
        "UPDATE ledger_legs SET owner = ?1, token = randomblob(16), lease_until = ?2, generation = ?3 WHERE id = ?4",
        params![std::process::id(),lease.until.to_be_bytes(),generation,leg],
    )?;
    let claim = started(transaction, leg, lease)?;
    Ok(Some(RetryDelivery {
        claim,
        identity: record.submission.identity,
        event: record.submission.event,
        leg: route,
    }))
}
