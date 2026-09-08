use super::StoreError;
use rusqlite::{Connection, TransactionBehavior};

pub(super) const VERSION: u32 = 3;

pub(super) fn validate(connection: &Connection) -> Result<u32, StoreError> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > VERSION {
        return Err(StoreError::UnsupportedSchema(version));
    }
    Ok(version)
}

pub(super) fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let version = validate(connection)?;
    if version == VERSION {
        return Ok(());
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // A second process may have migrated while this connection waited for ownership.
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 0 {
        transaction.execute_batch("CREATE TABLE legacy_imports (family TEXT PRIMARY KEY, error TEXT);
             CREATE TABLE presence (seq INTEGER PRIMARY KEY, line TEXT NOT NULL);
             CREATE TABLE policy_audit (seq INTEGER PRIMARY KEY, line TEXT NOT NULL);
             CREATE TABLE decisions (seq INTEGER PRIMARY KEY, line TEXT NOT NULL);
             CREATE TABLE return_claims (id INTEGER PRIMARY KEY AUTOINCREMENT, owner INTEGER NOT NULL);
             CREATE TABLE return_edge (id INTEGER PRIMARY KEY CHECK(id = 1), epoch BLOB NOT NULL CHECK(length(epoch) = 8));
             CREATE TABLE journal (seq INTEGER PRIMARY KEY, line TEXT NOT NULL, claim INTEGER REFERENCES return_claims(id));
             CREATE TABLE activity (seq INTEGER PRIMARY KEY, line TEXT NOT NULL);
             CREATE TABLE quiet (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
             CREATE TABLE staleness (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
             CREATE TABLE lights_complaint (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
             CREATE TABLE quiet_complaint (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
             CREATE TABLE lamp_news (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
             CREATE TABLE lamp_streak (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
             CREATE TABLE held_lamps (seq INTEGER PRIMARY KEY, token TEXT NOT NULL);
             CREATE TABLE lamp_mutes (seq INTEGER PRIMARY KEY, line TEXT NOT NULL);")?;
    } else if version > VERSION {
        return Err(StoreError::UnsupportedSchema(version));
    }
    if version < 2 {
        super::ledger::schema::create(&transaction)?;
    }
    if version < 3 {
        super::decisions::migrate(&transaction)?;
    }
    transaction.pragma_update(None, "user_version", VERSION)?;
    transaction.commit()?;
    Ok(())
}
