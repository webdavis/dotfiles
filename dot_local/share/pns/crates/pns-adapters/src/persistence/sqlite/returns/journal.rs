use super::StoreError;
use pns_application::{Claim, ReplayBatch, ReplayState, SubmissionIdentity};
use rusqlite::{Transaction, params};

pub(super) fn claim(
    transaction: &Transaction<'_>,
    since: Option<u64>,
    until: Option<u64>,
) -> Result<Option<(Claim, Option<i64>)>, StoreError> {
    let mut owners = transaction.prepare("SELECT id, owner FROM return_claims ORDER BY id")?;
    let owners = owners
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if owners
        .iter()
        .any(|(_, owner)| !crate::marker_files::owner_is_gone(&owner.to_string()))
    {
        return Ok(None);
    }
    let owned = if let Some((id, _)) = owners.first() {
        // Adoption retains the batch and its window. Joining later arrivals would
        // change the payload associated with an already submitted request key.
        transaction.execute(
            "UPDATE return_claims SET owner = ?1 WHERE id = ?2",
            params![std::process::id(), id],
        )?;
        *id
    } else {
        transaction.execute(
            "INSERT INTO return_claims(owner) VALUES (?1)",
            [std::process::id()],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute("UPDATE journal SET claim = ?1 WHERE claim IS NULL", [id])?;
        id
    };
    // Imported legacy holds have no key or saved window. Initialize those facts
    // once, in the same transaction that first adopts them into durable replay.
    transaction.execute(
        "UPDATE return_claims SET request_id = lower(hex(randomblob(16))), since = ?1, until = ?2
         WHERE id = ?3 AND request_id IS NULL",
        params![
            since.map(u64::to_be_bytes),
            until.map(u64::to_be_bytes),
            owned
        ],
    )?;
    let (request_id, since, until) = transaction.query_row(
        "SELECT request_id, since, until FROM return_claims WHERE id = ?1",
        [owned],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<[u8; 8]>>(1)?,
                row.get::<_, Option<[u8; 8]>>(2)?,
            ))
        },
    )?;
    let identity = SubmissionIdentity {
        producer: "pns-return".into(),
        request_id,
    };
    let queued = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM ledger_events WHERE producer = ?1 AND request_id = ?2)",
        params![identity.producer, identity.request_id],
        |row| row.get::<_, bool>(0),
    )?;
    let mut rows = transaction.prepare("SELECT line FROM journal WHERE claim = ?1 ORDER BY seq")?;
    let contents = rows
        .query_map([owned], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .concat();
    Ok(Some((
        Claim {
            since: since.map(u64::from_be_bytes),
            waiting: crate::journal_codec::entries(&contents),
            replay: Some(ReplayBatch {
                identity,
                until: until.map(u64::from_be_bytes),
                state: if queued {
                    ReplayState::Queued
                } else {
                    ReplayState::Unsubmitted
                },
            }),
        },
        Some(owned),
    )))
}
