use super::{SqliteStore, StoreError};
use pns_application::{DecisionOutcomes, SubmissionIdentity};
use pns_domain::{Delivery, Record};
use rusqlite::{OptionalExtension, Transaction, params};

pub(super) fn migrate(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(
        "ALTER TABLE decisions ADD COLUMN producer TEXT;
        ALTER TABLE decisions ADD COLUMN request_id TEXT
            CHECK ((producer IS NULL) = (request_id IS NULL));
        CREATE UNIQUE INDEX decision_identity ON decisions(producer, request_id);",
    )?;
    Ok(())
}

impl DecisionOutcomes for SqliteStore {
    fn begin(&self, identity: &SubmissionIdentity, record: &Record) -> Result<(), String> {
        self.transaction(|transaction| {
            let retained: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM decisions WHERE producer = ?1 AND request_id = ?2)",
                params![identity.producer, identity.request_id],
                |row| row.get(0),
            )?;
            if !retained {
                append(
                    transaction,
                    &crate::decision_codec::line(record),
                    Some(identity),
                )?;
            }
            Ok(())
        })
        .inspect_err(|error| self.report("decision", error))
        .map_err(|error| error.to_string())
    }
    fn revise(
        &self,
        identity: &SubmissionIdentity,
        destination: &str,
        delivery: &Delivery,
    ) -> Result<bool, String> {
        self.transaction(|transaction| revise(transaction, identity, destination, delivery))
            .inspect_err(|error| self.report("decision", error))
            .map_err(|error| error.to_string())
    }
}

pub(super) fn revise(
    transaction: &Transaction<'_>,
    identity: &SubmissionIdentity,
    destination: &str,
    delivery: &Delivery,
) -> Result<bool, StoreError> {
    let retained: Option<(i64, String)> = transaction
        .query_row(
            "SELECT seq, line FROM decisions WHERE producer = ?1 AND request_id = ?2",
            params![identity.producer, identity.request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((seq, line)) = retained else {
        return Ok(false);
    };
    let revised = crate::decision_codec::revise_leg(&line, destination, delivery)
        .map_err(|error| StoreError::InvalidState(error.into()))?;
    transaction.execute(
        "UPDATE decisions SET line = ?1 WHERE seq = ?2",
        params![revised, seq],
    )?;
    super::import::replaced(transaction, "decisions")?;
    Ok(true)
}

// This ring keeps row identity across appends because each destination outcome
// revises the same logical decision. Other rings retain their existing writer.
pub(super) fn append(
    transaction: &Transaction<'_>,
    line: &str,
    identity: Option<&SubmissionIdentity>,
) -> Result<(), StoreError> {
    // An imported empty file is distinguishable from absence until the first write.
    transaction.execute(
        "DELETE FROM decisions WHERE line = '' AND producer IS NULL",
        [],
    )?;
    let last: Option<(i64, String)> = transaction
        .query_row(
            "SELECT seq, line FROM decisions ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((seq, last)) = last
        && !last.ends_with('\n')
    {
        transaction.execute(
            "UPDATE decisions SET line = ?1 WHERE seq = ?2",
            params![format!("{last}\n"), seq],
        )?;
    }
    transaction.execute(
        "INSERT INTO decisions(line, producer, request_id) VALUES (?1, ?2, ?3)",
        params![
            format!("{line}\n"),
            identity.map(|id| &id.producer),
            identity.map(|id| &id.request_id)
        ],
    )?;
    let removed = transaction.execute("DELETE FROM decisions WHERE seq NOT IN (SELECT seq FROM decisions ORDER BY seq DESC LIMIT ?1)", [pns_domain::KEPT])?;
    if removed > 0 {
        // The legacy prune uses str::lines, normalizing CRLF only when pruning.
        let mut select = transaction.prepare("SELECT seq, line FROM decisions ORDER BY seq")?;
        let retained = select
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (seq, line) in retained {
            let normalized = format!("{}\n", line.lines().collect::<Vec<_>>().join("\n"));
            transaction.execute(
                "UPDATE decisions SET line = ?1 WHERE seq = ?2",
                params![normalized, seq],
            )?;
        }
    }
    super::import::replaced(transaction, "decisions")?;
    Ok(())
}
