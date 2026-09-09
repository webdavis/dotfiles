use crate::persistence::sqlite::StoreError;
use rusqlite::Transaction;

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE ledger_events (
          seq INTEGER PRIMARY KEY AUTOINCREMENT,
          producer TEXT NOT NULL, request_id TEXT NOT NULL,
          agent TEXT NOT NULL, state TEXT NOT NULL, project TEXT NOT NULL,
          branch TEXT NOT NULL, detail TEXT NOT NULL, title TEXT NOT NULL,
          message TEXT NOT NULL, preview TEXT NOT NULL, pane TEXT NOT NULL,
          UNIQUE(producer, request_id));
         CREATE TABLE ledger_legs (
          id INTEGER PRIMARY KEY, event INTEGER NOT NULL REFERENCES ledger_events(seq),
          position INTEGER NOT NULL, destination TEXT NOT NULL, route TEXT NOT NULL,
          mode INTEGER NOT NULL CHECK(mode IN (0,1)), decorative INTEGER NOT NULL CHECK(decorative IN (0,1)),
          generation INTEGER NOT NULL CHECK(generation > 0), owner INTEGER,
          token BLOB CHECK(token IS NULL OR length(token) = 16),
          lease_until BLOB CHECK(lease_until IS NULL OR length(lease_until) = 8),
          due BLOB NOT NULL CHECK(length(due) = 8),
          acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1)),
          UNIQUE(event, destination), UNIQUE(event, position));
         CREATE TABLE ledger_attempts (
          leg INTEGER NOT NULL REFERENCES ledger_legs(id), generation INTEGER NOT NULL,
          started BLOB NOT NULL CHECK(length(started) = 8),
          finished BLOB CHECK(finished IS NULL OR length(finished) = 8),
          outcome INTEGER NOT NULL CHECK(outcome IN (0,1,2,3)), detail TEXT NOT NULL,
          retry_at BLOB CHECK(retry_at IS NULL OR length(retry_at) = 8),
          PRIMARY KEY(leg, generation));"
    )?;
    Ok(())
}

pub(in crate::persistence::sqlite) fn retain_request(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch("ALTER TABLE ledger_events ADD COLUMN producer_request TEXT;")?;
    Ok(())
}

pub(in crate::persistence::sqlite) fn retain_deadletters(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch("ALTER TABLE ledger_legs ADD COLUMN deadlettered_at BLOB CHECK(deadlettered_at IS NULL OR length(deadlettered_at) = 8);
        ALTER TABLE ledger_legs ADD COLUMN deadletter_reason TEXT CHECK(deadletter_reason IN ('attempts','age','permanent'));
        CREATE TABLE delivery_health(id INTEGER PRIMARY KEY CHECK(id = 1), previous_pending INTEGER,
          growth INTEGER NOT NULL DEFAULT 0 CHECK(growth BETWEEN 0 AND 2),
          generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
          acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged >= 0 AND acknowledged <= generation));
        INSERT INTO delivery_health(id) VALUES (1);")?;
    Ok(())
}

pub(in crate::persistence::sqlite) fn retain_http_status(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch(&format!(
        "ALTER TABLE ledger_legs ADD COLUMN http_status INTEGER {STATUS_CHECK};
         ALTER TABLE ledger_attempts ADD COLUMN http_status INTEGER {STATUS_CHECK};",
        STATUS_CHECK = status_check("http_status")
    ))?;
    Ok(())
}

/// Any real HTTP status. Steps 6 and 7 wrote NARROWER constraints, four codes
/// and two dead-letter reasons, and step 8 widened them; both steps now write
/// the wide shape directly so a fresh database never builds the narrow one only
/// to rewrite it. Step 8 is what repairs a database that already ran the old
/// spellings, and it is skipped entirely on a fresh database.
fn status_check(column: &str) -> String {
    format!("CHECK({column} IS NULL OR {column} BETWEEN 100 AND 599)")
}

/// Widen ONE existing column's CHECK, on a database that already holds the
/// narrow spelling. SQLite has no `ALTER COLUMN`, and the documented way out, a
/// full table rebuild, is the wrong tool here: `ledger_attempts.leg` carries a
/// foreign key onto `ledger_legs(id)`, and dropping and recreating the parent of
/// a foreign key is exactly the case a rebuild mishandles. Adding the wide
/// column, copying, dropping the narrow one and renaming leaves both tables in
/// place and touches no key.
///
/// The cost is column ORDER: the widened column moves to the end of the row. No
/// query on either table uses `SELECT *`, so nothing reads a column by its
/// position in the table.
fn widen_column(
    transaction: &Transaction<'_>,
    table: &str,
    column: &str,
    check: &str,
) -> Result<(), StoreError> {
    transaction.execute_batch(&format!(
        "ALTER TABLE {table} ADD COLUMN widened {check};
         UPDATE {table} SET widened = {column};
         ALTER TABLE {table} DROP COLUMN {column};
         ALTER TABLE {table} RENAME COLUMN widened TO {column};"
    ))?;
    Ok(())
}

/// Migration step 8, the repair half: bring a database written under the narrow
/// spellings up to the shape steps 6 and 7 now write directly.
///
/// `from` is the version the database arrived at, NOT the version it is being
/// taken to, because that is what says which narrow columns it actually has: a
/// database at 6 has the two-value `deadletter_reason` and no `http_status` at
/// all, one at 7 has both narrow, and a fresh one has neither and does no work
/// here. That is the whole point of the split. Copying a table costs real time,
/// and a fresh database is what every test and every new machine creates.
pub(in crate::persistence::sqlite) fn widen_failure_checks(
    transaction: &Transaction<'_>,
    from: u32,
) -> Result<(), StoreError> {
    if from >= 6 {
        widen_column(
            transaction,
            "ledger_legs",
            "deadletter_reason",
            "TEXT CHECK(widened IN ('attempts','age','permanent'))",
        )?;
    }
    if from >= 7 {
        for table in ["ledger_legs", "ledger_attempts"] {
            widen_column(
                transaction,
                table,
                "http_status",
                &format!("INTEGER {}", status_check("widened")),
            )?;
        }
    }
    Ok(())
}
