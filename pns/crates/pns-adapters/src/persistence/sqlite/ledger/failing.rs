//! The failing legs, newest first: what `pns failures` lists and prints.
//!
//! A LEG IS FAILING WHEN ITS CURRENT GENERATION ENDED BADLY and it has not been
//! acknowledged. That is the honest reading of "what is not arriving": a leg
//! that failed once and later succeeded is history rather than a problem, and
//! its acknowledgement is what says so.

use super::*;
use pns_application::{RetryFacts, StoredFailure};
use pns_domain::retry::TransportOutcome;
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
        (Some(code), _) => TransportOutcome::Status(code),
        // Unlaunched: the request was never put on the wire, so it carries no
        // status by construction rather than by loss.
        (None, 3) => TransportOutcome::NoStatus,
        (None, _) => TransportOutcome::NoResponse,
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

/// The two ledger facts a retrying leg's "next try" and "retry deadline" come
/// from, for the id a listing showed. `None` when that id names no leg at
/// all, the same as [`one`].
pub(super) fn retry_facts(
    connection: &Connection,
    id: i64,
) -> Result<Option<RetryFacts>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT due, (SELECT started FROM ledger_attempts
              WHERE leg = ledger_legs.id AND generation = 1)
             FROM ledger_legs WHERE id = ?1",
            [id],
            |row| {
                Ok(RetryFacts {
                    due: u64::from_be_bytes(row.get(0)?),
                    started: u64::from_be_bytes(row.get(1)?),
                })
            },
        )
        .optional()?)
}

/// Every distinct route the ledger has ever posted to, plus nothing else.
///
/// THE LEDGER IS THE ONLY ROSTER pns has. Routes arrive from producers as
/// `--route <name>` at call time and are written down nowhere else: the
/// gateway's own route table lives in its config, which pns does not read and
/// which holds that gateway's secrets. So the set worth checking is the set pns
/// has actually used, which is also exactly the set that can already have lost
/// a page.
pub(super) fn routes(connection: &Connection) -> Result<Vec<String>, StoreError> {
    let mut query = connection
        .prepare("SELECT DISTINCT route FROM ledger_legs WHERE destination = 'hermes' AND route <> '' ORDER BY route")?;
    let routes = query
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(routes)
}

/// How many failing legs the retry policy has given up on, unbounded: a
/// COUNT over the same WHERE clause `newest` filters to, rather than a
/// row scan capped at some depth that could hide older ones behind a
/// backlog of still-retrying legs.
pub(super) fn dead_lettered_count(connection: &Connection) -> Result<u64, StoreError> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM ledger_legs WHERE acknowledged = 0 AND deadlettered_at IS NOT NULL",
        [],
        |row| row.get(0),
    )?)
}
