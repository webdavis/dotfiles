use super::Sandbox;
use rusqlite::{Connection, OpenFlags};
use std::time::Duration;

pub(super) fn database(sandbox: &Sandbox) -> Connection {
    at(&sandbox.path("state/pns.db"))
}

pub(super) fn at(path: &std::path::Path) -> Connection {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .expect("the real stored database");
    connection
        .busy_timeout(Duration::from_millis(25))
        .expect("bounded observation");
    connection
}

// Shared by acceptance targets that observe different record families.
#[allow(dead_code)]
pub(super) fn lines(sandbox: &Sandbox, table: &str) -> Vec<String> {
    text(sandbox, table).lines().map(str::to_owned).collect()
}

// Shared by acceptance targets that observe different record families.
#[allow(dead_code)]
pub(super) fn text(sandbox: &Sandbox, table: &str) -> String {
    if !sandbox.path("state/pns.db").exists() {
        return String::new();
    }
    let connection = database(sandbox);
    let pending = if table == "journal" {
        " WHERE claim IS NULL"
    } else {
        ""
    };
    connection
        .prepare(&format!("SELECT line FROM {table}{pending} ORDER BY seq"))
        .expect("the existing record table")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("read stored rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("stored line text")
        .concat()
}

// Shared by acceptance targets that observe different record families.
#[allow(dead_code)]
pub(super) fn claims(sandbox: &Sandbox) -> Vec<(i64, u32, String)> {
    database(sandbox)
        .prepare("SELECT c.id, c.owner, j.line FROM return_claims c JOIN journal j ON j.claim = c.id ORDER BY c.id, j.seq")
        .expect("the claim rows")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("read claimed bytes and their owner")
        .collect::<rusqlite::Result<_>>()
        .expect("claimed records")
}

// Shared by acceptance targets that observe different record families.
#[allow(dead_code)]
pub(super) fn assert_consumed(sandbox: &Sandbox) {
    let remaining: (u64, u64) = database(sandbox)
        .query_row(
            "SELECT (SELECT count(*) FROM journal), (SELECT count(*) FROM return_claims)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the real pending and claimed rows");
    assert_eq!(remaining, (0, 0), "the replay left journal or claim rows");
}

// Shared by acceptance targets that observe different record families.
#[allow(dead_code)]
pub(super) fn present(sandbox: &Sandbox) -> Option<u64> {
    use rusqlite::OptionalExtension;
    if !sandbox.path("state/pns.db").exists() {
        return None;
    }
    database(sandbox)
        .query_row("SELECT epoch FROM return_edge WHERE id = 1", [], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .optional()
        .expect("read stored return edge")
        .map(|bytes| u64::from_be_bytes(bytes.try_into().expect("eight-byte epoch")))
}
