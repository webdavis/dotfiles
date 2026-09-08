use super::*;
use pns_domain::{Event, routing::ReportMode};
use rusqlite::{Connection, OptionalExtension, Row};

pub(super) fn find(
    connection: &Connection,
    identity: &SubmissionIdentity,
) -> Result<Option<SubmissionRecord>, StoreError> {
    let sequence = connection
        .query_row(
            "SELECT seq FROM ledger_events WHERE producer = ?1 AND request_id = ?2",
            [&identity.producer, &identity.request_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    sequence.map(|seq| record(connection, seq)).transpose()
}
pub(super) fn record(
    connection: &Connection,
    sequence: i64,
) -> Result<SubmissionRecord, StoreError> {
    let (identity, event) = connection.query_row(
        "SELECT producer, request_id, agent, state, project, branch, detail, title, message, preview, pane
         FROM ledger_events WHERE seq = ?1", [sequence], |row| Ok((
            SubmissionIdentity { producer: row.get(0)?, request_id: row.get(1)? },
            Event { agent: row.get(2)?, state: row.get(3)?, project: row.get(4)?, branch: row.get(5)?,
                detail: row.get(6)?, title: row.get(7)?, message: row.get(8)?, preview: row.get(9)?, pane: row.get(10)? }
        )),
    )?;
    let mut query = connection.prepare(
        "SELECT destination, route, mode, decorative FROM ledger_legs WHERE event = ?1 ORDER BY position")?;
    let legs = query
        .query_map([sequence], leg)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut query = connection.prepare(
        "SELECT l.destination, a.generation, COALESCE(a.finished, a.started), a.outcome, a.detail, a.retry_at
         FROM ledger_attempts a JOIN ledger_legs l ON l.id = a.leg WHERE l.event = ?1
         ORDER BY a.generation, l.position")?;
    let attempts = query
        .query_map([sequence], |row| {
            Ok(LegAttempt {
                destination: row.get(0)?,
                generation: row.get(1)?,
                at: u64::from_be_bytes(row.get(2)?),
                completion: completion(row)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SubmissionRecord {
        sequence: u64::try_from(sequence)
            .map_err(|_| StoreError::InvalidState("invalid ledger sequence".into()))?,
        submission: LedgerSubmission {
            identity,
            event,
            legs,
        },
        attempts,
    })
}
pub(super) fn leg(row: &Row<'_>) -> rusqlite::Result<LedgerLeg> {
    Ok(LedgerLeg {
        destination: row.get(0)?,
        route: row.get(1)?,
        mode: match row.get::<_, u8>(2)? {
            0 => ReportMode::Silent,
            1 => ReportMode::ReportOutcome,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        decorative: row.get(3)?,
    })
}
fn completion(row: &Row<'_>) -> rusqlite::Result<LedgerCompletion> {
    let detail = row.get(4)?;
    let outcome = match row.get::<_, u8>(3)? {
        1 => return Ok(LedgerCompletion::Acknowledged { detail }),
        0 => UnconfirmedDelivery::Unknown,
        2 => UnconfirmedDelivery::Failed,
        3 => UnconfirmedDelivery::Unlaunched,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(LedgerCompletion::Retry {
        outcome,
        detail,
        retry_at: u64::from_be_bytes(row.get(5)?),
    })
}
