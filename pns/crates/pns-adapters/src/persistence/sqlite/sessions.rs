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

// --- the wait, and the escalation about one ----------------------------------

impl SqliteStore {
    /// Start this session's wait on the operator.
    ///
    /// IT UPSERTS rather than updating, so a wait is recorded even for a
    /// session whose row nothing wrote first. Every hook path names the
    /// session before it raises an event, so the row is normally already
    /// there; an UPDATE that matched nothing would leave the escalation
    /// silently dead for that session instead.
    ///
    /// AND IT CLEARS THE PREVIOUS ESCALATION, which is what makes the rule one
    /// page per BLOCK rather than one per session: a new wait is a new thing
    /// nobody has answered, whether or not the last one was ever paged about.
    pub fn begin_wait(&self, session_id: &str, now: u64) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute(
                "INSERT INTO sessions(id,harness,project,branch,title,first_seen,last_seen,blocked_since)
                 VALUES (?1,'','','','',?2,?2,?2)
                 ON CONFLICT(id) DO UPDATE SET
                   blocked_since = ?2,
                   escalated_at = NULL,
                   last_seen = ?2",
                rusqlite::params![session_id, now],
            )?;
            Ok(())
        })
    }

    /// End it. NOTHING IS INSERTED: a session with no row has no wait to end,
    /// and the escalation reads the row rather than this call's success.
    pub fn end_wait(&self, session_id: &str) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute(
                "UPDATE sessions SET blocked_since = NULL, escalated_at = NULL WHERE id = ?1",
                rusqlite::params![session_id],
            )?;
            Ok(())
        })
    }

    /// Every session still waiting since `threshold` or earlier that has not
    /// been escalated about, oldest first.
    ///
    /// THE THRESHOLD IS THE CALLER'S SUM (`now - window`), so the subtraction
    /// that could underflow is done once, in the fire, rather than inside SQL
    /// where a negative would quietly select everything.
    pub fn stale_blocks(
        &self,
        threshold: u64,
    ) -> Result<Vec<pns_domain::stale::Blocked>, StoreError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, harness, project, branch, title, blocked_since
               FROM sessions
              WHERE blocked_since IS NOT NULL
                AND blocked_since <= ?1
                AND escalated_at IS NULL
              ORDER BY blocked_since",
        )?;
        let rows = statement.query_map(rusqlite::params![threshold], |row| {
            Ok(pns_domain::stale::Blocked {
                session: row.get(0)?,
                harness: row.get(1)?,
                project: row.get(2)?,
                branch: row.get(3)?,
                title: row.get(4)?,
                since: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Stamp this session's escalation, answering whether THIS caller stamped
    /// it.
    ///
    /// THE STAMP IS THE CLAIM. `escalated_at IS NULL` inside the write makes
    /// it a compare-and-swap that SQLite arbitrates, so two fires woken in one
    /// tick produce one page between them without a lock file of their own.
    ///
    /// STAMPED ON ATTEMPT, NEVER ON SUCCESS, which matches the reminder's own
    /// honesty: a mute, a Focus or an empty plan can suppress the delivery,
    /// and a page that retried every hour because the first one was muted is
    /// the failure mode worth avoiding.
    pub fn claim_escalation(&self, session_id: &str, now: u64) -> Result<bool, StoreError> {
        self.transaction(|transaction| {
            Ok(transaction.execute(
                "UPDATE sessions SET escalated_at = ?2
                  WHERE id = ?1 AND escalated_at IS NULL AND blocked_since IS NOT NULL",
                rusqlite::params![session_id, now],
            )? == 1)
        })
    }
}

// --- the wait a report reads -------------------------------------------------

impl SqliteStore {
    /// The session waiting on the operator now, the newest wait first.
    ///
    /// NEWEST RATHER THAN OLDEST, where `stale_blocks` takes the oldest: the
    /// escalation pages about the wait that has gone unanswered longest, and a
    /// report answering "where was I" means the one they walked away from.
    ///
    /// EVERY WAIT, ESCALATED OR NOT. A page already sent about a block does
    /// not make the block answered, and this read arms nothing.
    ///
    /// READ ONLY, AND EVERY FAILURE IS NO WAIT AT ALL. A report must not
    /// create, import or migrate a database, and a store it cannot open says
    /// nothing rather than refusing to print.
    pub fn newest_wait(&self) -> Option<pns_domain::stale::Blocked> {
        let connection = self.read_only().ok()?;
        connection
            .query_row(
                "SELECT id, harness, project, branch, title, blocked_since
                   FROM sessions
                  WHERE blocked_since IS NOT NULL
                  ORDER BY blocked_since DESC, id
                  LIMIT 1",
                [],
                |row| {
                    Ok(pns_domain::stale::Blocked {
                        session: row.get(0)?,
                        harness: row.get(1)?,
                        project: row.get(2)?,
                        branch: row.get(3)?,
                        title: row.get(4)?,
                        since: row.get(5)?,
                    })
                },
            )
            .ok()
    }
}
