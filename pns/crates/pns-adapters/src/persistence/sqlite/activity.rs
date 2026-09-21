//! The activity store: one durable row per harness hook event, in the same
//! database as the ledger.
//!
//! IT IS NOT THE ACTIVITY RING BESIDE IT. The ring keeps a bounded tail of
//! rendered lines for the return card and rolls off by COUNT; this table keeps
//! the fields themselves and rolls off by AGE, so a recap can ask what a
//! window held rather than what the last few dozen events were.
//!
//! THE CLOCK IS EPOCH SECONDS, like every other time column in this database
//! (`sessions.first_seen`, the ledger's own stamps). A prune is then an integer
//! comparison SQLite can index, and the reader formats the local time it
//! prints, which is the only place a zone offset is known.
//!
//! A WRITE THAT FAILS COSTS ONE ROW AND NOTHING ELSE. The caller is a hook on
//! the notification path, so the error is reported and the event carries on.

use super::{SqliteStore, StoreError};
use rusqlite::Transaction;

/// One hook event, as the recap reads it back. THE TYPE IS THE DOMAIN'S,
/// because grouping these into sessions is policy and a domain that could not
/// name the fields would have to be handed a shape this table invented.
pub use pns_domain::recap::activity::Event as ActivityEvent;

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    // The index is the prune's own, and the window read takes the same order.
    transaction.execute_batch(
        "CREATE TABLE activity_events (
          seq INTEGER PRIMARY KEY AUTOINCREMENT,
          at INTEGER NOT NULL,
          agent TEXT NOT NULL,
          state TEXT NOT NULL,
          project TEXT NOT NULL,
          branch TEXT NOT NULL,
          session TEXT NOT NULL,
          session_title TEXT NOT NULL,
          pane TEXT NOT NULL,
          workspace TEXT NOT NULL,
          model TEXT NOT NULL,
          title TEXT NOT NULL,
          detail TEXT NOT NULL,
          transcript_path TEXT NOT NULL DEFAULT '');
         CREATE INDEX activity_events_at ON activity_events(at);",
    )?;
    Ok(())
}

/// Add the transcript path to a table created before it existed.
///
/// SKIPPED ON A DATABASE THAT NEVER HELD THE OLD SHAPE, because `create` above
/// already writes the column: a fresh machine runs both migrations in one
/// transaction and `ALTER TABLE` would then refuse a column that is already
/// there.
pub(in crate::persistence::sqlite) fn transcript_path(
    transaction: &Transaction<'_>,
    from_version: u32,
) -> Result<(), StoreError> {
    if from_version >= 11 {
        transaction.execute_batch(
            "ALTER TABLE activity_events ADD COLUMN transcript_path TEXT NOT NULL DEFAULT '';",
        )?;
    }
    Ok(())
}

impl SqliteStore {
    /// Record one hook event.
    pub fn record_activity_event(&self, event: &ActivityEvent) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute(
                "INSERT INTO activity_events(
                   at,agent,state,project,branch,session,session_title,pane,workspace,model,title,detail,transcript_path)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                rusqlite::params![
                    event.at,
                    event.agent,
                    event.state,
                    event.project,
                    event.branch,
                    event.session,
                    event.session_title,
                    event.pane,
                    event.workspace,
                    event.model,
                    event.title,
                    event.detail,
                    event.transcript_path
                ],
            )?;
            Ok(())
        })
    }

    /// Delete every row older than `cutoff`, answering how many went.
    ///
    /// THE CUTOFF IS THE CALLER'S SUBTRACTION (`now - retain`), for
    /// `stale_blocks`' reason: a subtraction that could underflow is done once
    /// where a clock is in hand rather than inside SQL, where a negative would
    /// quietly match nothing.
    ///
    /// STRICTLY OLDER, so a row written exactly `retain` ago survives one more
    /// tick rather than being deleted on the boundary it is still inside.
    pub fn prune_activity(&self, cutoff: u64) -> Result<usize, StoreError> {
        self.transaction(|transaction| {
            Ok(transaction.execute(
                "DELETE FROM activity_events WHERE at < ?1",
                rusqlite::params![cutoff],
            )?)
        })
    }

    /// Every event stamped after `since` and at or before `until`, oldest
    /// first, which is the order a recap prints them in.
    pub fn activity_between(
        &self,
        since: u64,
        until: u64,
    ) -> Result<Vec<ActivityEvent>, StoreError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT at,agent,state,project,branch,session,session_title,pane,workspace,model,title,detail,transcript_path
               FROM activity_events
              WHERE at > ?1 AND at <= ?2
              ORDER BY at, seq",
        )?;
        let rows = statement.query_map(rusqlite::params![since, until], |row| {
            Ok(ActivityEvent {
                at: row.get(0)?,
                agent: row.get(1)?,
                state: row.get(2)?,
                project: row.get(3)?,
                branch: row.get(4)?,
                session: row.get(5)?,
                session_title: row.get(6)?,
                pane: row.get(7)?,
                workspace: row.get(8)?,
                model: row.get(9)?,
                title: row.get(10)?,
                detail: row.get(11)?,
                transcript_path: row.get(12)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// The activity table as the recap engine reads it.
///
/// A WINDOW THAT WILL NOT READ IS EMPTY HERE, and the composition root is what
/// refuses: a store that cannot be opened is a recap with no agents section,
/// which the design says is not a recap, so `pns recap` checks the store
/// before it composes anything.
impl pns_application::ActivityEvents for SqliteStore {
    fn activity_between(&self, since: u64, until: u64) -> Vec<ActivityEvent> {
        SqliteStore::activity_between(self, since, until).unwrap_or_default()
    }
}
