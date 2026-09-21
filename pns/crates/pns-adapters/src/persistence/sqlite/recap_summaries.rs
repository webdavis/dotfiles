//! The summaries the gateway pregenerated, one row per window.
//!
//! ONE ROW PER WINDOW NAME, replaced in place: a window has one current
//! summary, and keeping the older ones would be a log nobody reads. `at` is
//! epoch seconds like every other time column here, and `covers` is the stamp
//! of the newest event the summary was written over, which is what makes a
//! stored summary older than the window's last event detectable without
//! re-reading the window.

use super::{SqliteStore, StoreError};
use rusqlite::Transaction;

/// One stored summary as the recap reads it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSummary {
    pub at: u64,
    pub covers: u64,
    pub source: String,
    pub text: String,
}

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE recap_summaries (
           window TEXT PRIMARY KEY,
           at INTEGER NOT NULL,
           covers INTEGER NOT NULL,
           source TEXT NOT NULL,
           text TEXT NOT NULL);",
    )?;
    Ok(())
}

impl SqliteStore {
    /// Write this window's summary, replacing whatever it held.
    pub fn store_recap_summary(
        &self,
        window: &str,
        summary: &StoredSummary,
    ) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute(
                "INSERT INTO recap_summaries(window,at,covers,source,text)
                 VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(window) DO UPDATE SET
                   at=excluded.at, covers=excluded.covers,
                   source=excluded.source, text=excluded.text",
                rusqlite::params![
                    window,
                    summary.at,
                    summary.covers,
                    summary.source,
                    summary.text
                ],
            )?;
            Ok(())
        })
    }

    /// This window's stored summary, or None when it has none.
    pub fn recap_summary(&self, window: &str) -> Result<Option<StoredSummary>, StoreError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT at,covers,source,text FROM recap_summaries WHERE window = ?1")?;
        let mut rows = statement.query_map(rusqlite::params![window], |row| {
            Ok(StoredSummary {
                at: row.get(0)?,
                covers: row.get(1)?,
                source: row.get(2)?,
                text: row.get(3)?,
            })
        })?;
        rows.next().transpose().map_err(StoreError::from)
    }

    /// Delete every summary written before `cutoff`, which is the activity
    /// store's own retention applied to the paragraphs about it.
    pub fn prune_recap_summaries(&self, cutoff: u64) -> Result<usize, StoreError> {
        self.transaction(|transaction| {
            Ok(transaction.execute(
                "DELETE FROM recap_summaries WHERE at < ?1",
                rusqlite::params![cutoff],
            )?)
        })
    }
}
