//! The sessions a harness has run: who sent an event, and what the session
//! was asked to do.
//!
//! ONE ROW PER SESSION, REPLACED IN PLACE, which is why this is a table and
//! not a file per session under the state directory: every marker family
//! there carries a sweeper, and a row that is overwritten needs none.
//!
//! ONE ROW PER SESSION FOREVER IS THE ACCEPTED COST, unlike `ledger_events`,
//! which carries three retention settings. A row is a few hundred bytes, so
//! a machine running dozens of sessions a day adds single-digit megabytes a
//! year, and a sweep would be worse than the bytes it saves: an operator
//! resuming a session past the window would lose the title that names it,
//! because only the FIRST prompt of a session carries one and no later event
//! can write it back.

use super::{SqliteStore, StoreError};
use rusqlite::Transaction;

/// What an event knows about its own session. Empty fields are a caller with
/// nothing to say rather than an erasure: the prompt hook knows the title and
/// spawns no git, and every later event knows the checkout and no title.
pub struct SessionNote<'a> {
    pub id: &'a str,
    pub harness: &'a str,
    pub project: &'a str,
    pub branch: &'a str,
    pub title: &'a str,
    pub now: u64,
}

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    // `blocked_since` and `escalated_at` are created here and written by the
    // stale-block escalation, so that feature costs no second migration.
    transaction.execute_batch(
        "CREATE TABLE sessions (
          id TEXT PRIMARY KEY,
          harness TEXT NOT NULL,
          project TEXT NOT NULL,
          branch TEXT NOT NULL,
          title TEXT NOT NULL,
          first_seen INTEGER NOT NULL,
          last_seen INTEGER NOT NULL,
          blocked_since INTEGER,
          escalated_at INTEGER);",
    )?;
    Ok(())
}

impl SqliteStore {
    /// Record that this session is alive, and answer the title it is known
    /// by.
    ///
    /// THE TITLE IS WRITTEN ONCE. The first prompt of a session is the one
    /// that names it, so a later prompt does not relabel every event that
    /// came before; every other field is refreshed when the caller has one,
    /// because a session can change branch mid-flight.
    pub fn note_session(&self, session: &SessionNote<'_>) -> Result<String, StoreError> {
        self.transaction(|transaction| {
            Ok(transaction.query_row(
                "INSERT INTO sessions(id,harness,project,branch,title,first_seen,last_seen)
                 VALUES (?1,?2,?3,?4,?5,?6,?6)
                 ON CONFLICT(id) DO UPDATE SET
                   harness = CASE WHEN ?2 = '' THEN sessions.harness ELSE ?2 END,
                   project = CASE WHEN ?3 = '' THEN sessions.project ELSE ?3 END,
                   branch = CASE WHEN ?4 = '' THEN sessions.branch ELSE ?4 END,
                   title = CASE WHEN sessions.title = '' THEN ?5 ELSE sessions.title END,
                   last_seen = ?6
                 RETURNING title",
                rusqlite::params![
                    session.id,
                    session.harness,
                    session.project,
                    session.branch,
                    session.title,
                    session.now
                ],
                |row| row.get(0),
            )?)
        })
    }
}
