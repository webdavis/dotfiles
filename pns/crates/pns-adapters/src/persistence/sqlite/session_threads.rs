//! The thread a session already owns in a channel, one row per pair.
//!
//! KEYED ON THE PAIR, NOT THE SESSION, which is what makes a session that
//! spans two repositories correct rather than surprising: an agent that
//! starts in one checkout and posts an event from another resolves a second
//! channel, finds no row for that pair, and opens a second thread there. A
//! single column on `sessions` would have posted the second repository's
//! events into the first repository's channel.
//!
//! ONE ROW PER PAIR FOREVER IS THE ACCEPTED COST, same reasoning as
//! `sessions`: the key is the (session, channel) pair, a session id is
//! unique per session, so a new session never overwrites an old row and
//! rows accumulate one per pair for good. A row is a few dozen bytes, so the
//! accumulation costs single-digit megabytes a year even at heavy use, and a
//! sweeper would cost more to build and run than the bytes it saves.
//!
//! A STORE FAILURE IS NOT A DELIVERY FAILURE. Every method here answers in
//! the shape the destination can act on and swallows the error: the worst a
//! dead database can do to an event is cost it a thread, and an event posted
//! at channel level beats an event not posted at all.

use super::{SqliteStore, StoreError};
use crate::destinations::discord::SessionThreads;
use rusqlite::Transaction;

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE session_threads (
          session TEXT NOT NULL,
          channel TEXT NOT NULL,
          thread TEXT NOT NULL,
          PRIMARY KEY (session, channel));",
    )?;
    Ok(())
}

impl SessionThreads for SqliteStore {
    fn thread(&self, session: &str, channel: &str) -> Option<String> {
        self.transaction(|transaction| {
            Ok(transaction
                .query_row(
                    "SELECT thread FROM session_threads WHERE session = ?1 AND channel = ?2",
                    rusqlite::params![session, channel],
                    |row| row.get::<_, String>(0),
                )
                .ok())
        })
        .ok()
        .flatten()
    }

    fn remember(&self, session: &str, channel: &str, thread: &str) {
        // FIRST WRITER WINS, ON PURPOSE: `remember` only ever runs on a pair
        // with no row (opened fresh, or forgotten first), so a conflict here
        // means two concurrent `opening` calls raced and both created a
        // thread. DO NOTHING keeps the earlier row instead of clobbering it,
        // so every later lookup for the pair converges on one thread instead
        // of flapping between the two the race created.
        let _ = self.transaction(|transaction| {
            transaction.execute(
                "INSERT INTO session_threads(session,channel,thread) VALUES (?1,?2,?3)
                 ON CONFLICT(session,channel) DO NOTHING",
                rusqlite::params![session, channel, thread],
            )?;
            Ok(())
        });
    }

    fn forget(&self, session: &str, channel: &str) {
        let _ = self.transaction(|transaction| {
            transaction.execute(
                "DELETE FROM session_threads WHERE session = ?1 AND channel = ?2",
                rusqlite::params![session, channel],
            )?;
            Ok(())
        });
    }
}
