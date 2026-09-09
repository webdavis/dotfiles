//! The failing legs, newest first: what `pns failures` lists and prints.
//!
//! A LEG IS FAILING WHEN ITS CURRENT GENERATION ENDED BADLY and it has not been
//! acknowledged. That is the honest reading of "what is not arriving": a leg
//! that failed once and later succeeded is history rather than a problem, and
//! its acknowledgement is what says so.

use super::*;
use pns_application::StoredFailure;
use pns_domain::retry::DeliveryOutcome;
use rusqlite::{Connection, OptionalExtension, Row};

/// The newest `limit` failing legs.
///
/// Ordered by WHEN THE ATTEMPT FINISHED and then by id, because two legs of one
/// event finish in the same second and a listing that reordered itself between
/// two runs would make an id unusable for the second command.
pub(super) fn newest(
    connection: &Connection,
    limit: u32,
) -> Result<Vec<StoredFailure>, StoreError> {
    let mut query = connection.prepare(&format!(
        "{SELECT} ORDER BY finished DESC, l.id DESC LIMIT ?1"
    ))?;
    let failures = query
        .query_map([limit], row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(failures)
}

/// One failing leg by the id a listing showed.
pub(super) fn one(connection: &Connection, id: i64) -> Result<Option<StoredFailure>, StoreError> {
    Ok(connection
        .query_row(&format!("{SELECT} AND l.id = ?1"), [id], row)
        .optional()?)
}

/// The one query both read paths share, so a listing and a detail view cannot
/// disagree about which legs are failing.
///
/// The attempt joined is the leg's CURRENT generation, which is the last thing
/// that happened to it. Outcomes 2 and 3 are the two failure spellings; 0 is a
/// channel that ran and said nothing and 1 is an acknowledgement.
const SELECT: &str = "SELECT l.id, l.destination, l.route, e.agent, e.state,
     l.deadlettered_at IS NOT NULL, l.generation, a.http_status, a.outcome,
     COALESCE(a.finished, a.started) AS finished
   FROM ledger_legs l
   JOIN ledger_events e ON e.seq = l.event
   JOIN ledger_attempts a ON a.leg = l.id AND a.generation = l.generation
   WHERE l.acknowledged = 0 AND a.outcome IN (2,3)";

fn row(row: &Row<'_>) -> rusqlite::Result<StoredFailure> {
    let status: Option<u16> = row.get(7)?;
    let outcome = match (status, row.get::<_, u8>(8)?) {
        (Some(code), _) => DeliveryOutcome::Status(code),
        // Unlaunched: the request was never put on the wire, so it carries no
        // status by construction rather than by loss.
        (None, 3) => DeliveryOutcome::NoStatus,
        (None, _) => DeliveryOutcome::NoResponse,
    };
    let generation: u64 = row.get(6)?;
    Ok(StoredFailure {
        id: row.get::<_, i64>(0)? as u64,
        destination: row.get(1)?,
        route: row.get(2)?,
        agent: row.get(3)?,
        state: row.get(4)?,
        deadlettered: row.get(5)?,
        // Generation 1 is the initial send, which consumes no retry.
        retries: generation.saturating_sub(1),
        outcome,
        failed_at: u64::from_be_bytes(row.get(9)?),
    })
}
