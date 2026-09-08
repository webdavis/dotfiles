use super::{Sandbox, stored_records};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::time::Duration;

pub(super) fn expiry(sandbox: &Sandbox) -> Option<u64> {
    stored_records::database(sandbox)
        .query_row("SELECT body FROM quiet WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .expect("the actual quiet row")
        .map(|body| body.parse().expect("one stored epoch second"))
}

pub(super) fn held(sandbox: &Sandbox) -> Vec<String> {
    stored_records::database(sandbox)
        .prepare("SELECT token FROM held_lamps ORDER BY seq")
        .expect("the held-lamp table")
        .query_map([], |row| row.get(0))
        .expect("the actual held rows")
        .collect::<rusqlite::Result<_>>()
        .expect("stored lamp tokens")
}

pub(super) fn writer(sandbox: &Sandbox) -> Connection {
    let connection = Connection::open_with_flags(
        sandbox.path("state/pns.db"),
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .expect("the initialized private database");
    connection
        .busy_timeout(Duration::from_millis(25))
        .expect("bounded ownership wait");
    connection
        .execute_batch("BEGIN IMMEDIATE")
        .expect("the competing writer owns publication");
    connection
}
