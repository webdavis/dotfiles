use super::StoreError;
use rusqlite::Transaction;

pub(in super::super) fn migrate(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(
        "ALTER TABLE return_claims ADD COLUMN request_id TEXT;
         ALTER TABLE return_claims ADD COLUMN since BLOB;
         ALTER TABLE return_claims ADD COLUMN until BLOB;
         ALTER TABLE journal ADD COLUMN producer TEXT;
         ALTER TABLE journal ADD COLUMN request_id TEXT;
         CREATE UNIQUE INDEX journal_identity ON journal(producer, request_id)
             WHERE producer IS NOT NULL AND request_id IS NOT NULL;",
    )?;
    Ok(())
}
