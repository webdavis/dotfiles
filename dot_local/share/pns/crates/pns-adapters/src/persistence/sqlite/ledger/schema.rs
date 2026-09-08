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
        ALTER TABLE ledger_legs ADD COLUMN deadletter_reason TEXT CHECK(deadletter_reason IN ('attempts','age'));
        CREATE TABLE delivery_health(id INTEGER PRIMARY KEY CHECK(id = 1), previous_pending INTEGER,
          growth INTEGER NOT NULL DEFAULT 0 CHECK(growth BETWEEN 0 AND 2),
          generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
          acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged >= 0 AND acknowledged <= generation));
        INSERT INTO delivery_health(id) VALUES (1);")?;
    Ok(())
}
