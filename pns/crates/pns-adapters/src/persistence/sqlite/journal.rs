use super::StoreError;
use pns_application::SubmissionIdentity;
use pns_domain::EventArgs;
use rusqlite::{OptionalExtension, Transaction, params};

pub(super) fn append(
    transaction: &Transaction<'_>,
    event: &EventArgs,
    now: Option<u64>,
    identity: Option<&SubmissionIdentity>,
) -> Result<(), StoreError> {
    if let Some(identity) = identity {
        let recorded: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM journal WHERE producer = ?1 AND request_id = ?2)
             OR EXISTS(SELECT 1 FROM ledger_events e JOIN ledger_legs l ON l.event = e.seq
                 WHERE e.producer = ?1 AND e.request_id = ?2 AND l.decorative = 1 AND l.acknowledged = 1)",
            params![identity.producer, identity.request_id], |row| row.get(0),
        )?;
        // Completion and this append serialize through the same writer. A late
        // event tail cannot resurrect a miss the daemon already acknowledged.
        if recorded {
            return Ok(());
        }
    }
    let tail = transaction
        .query_row(
            "SELECT seq, line FROM journal WHERE claim IS NULL ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((seq, line)) = tail.filter(|(_, line)| !line.is_empty() && !line.ends_with('\n')) {
        transaction.execute(
            "UPDATE journal SET line = ?1 WHERE seq = ?2",
            params![format!("{line}\n"), seq],
        )?;
    }
    let line = crate::journal_codec::entry(event, now, pns_domain::render::PREVIEW_MAX_CHARS);
    transaction.execute(
        "INSERT INTO journal(line, producer, request_id) VALUES (?1, ?2, ?3)",
        params![
            format!("{line}\n"),
            identity.map(|id| &id.producer),
            identity.map(|id| &id.request_id)
        ],
    )?;
    let count: usize = transaction.query_row(
        "SELECT count(*) FROM journal WHERE claim IS NULL",
        [],
        |row| row.get(0),
    )?;
    if count > pns_domain::missed::KEPT {
        transaction.execute(
            "DELETE FROM journal WHERE claim IS NULL AND seq NOT IN
             (SELECT seq FROM journal WHERE claim IS NULL ORDER BY seq DESC LIMIT ?1)",
            [pns_domain::missed::KEPT],
        )?;
        // The legacy writer normalizes retained line endings only when pruning.
        // Preserve that byte rule while keeping each row's original identity.
        let mut rows = transaction.prepare("SELECT seq, line FROM journal WHERE claim IS NULL")?;
        let lines = rows
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (seq, line) in lines {
            transaction.execute(
                "UPDATE journal SET line = ?1 WHERE seq = ?2",
                params![format!("{}\n", line.lines().next().unwrap_or("")), seq],
            )?;
        }
    }
    super::import::replaced(transaction, crate::MISSED_NOTIFICATIONS)?;
    Ok(())
}

pub(super) fn acknowledge(
    transaction: &Transaction<'_>,
    identity: &SubmissionIdentity,
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM journal WHERE producer = ?1 AND request_id = ?2",
        params![identity.producer, identity.request_id],
    )?;
    Ok(())
}
