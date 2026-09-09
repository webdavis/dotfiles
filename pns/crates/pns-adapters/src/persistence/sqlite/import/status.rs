use super::{super::StoreError, ImportFailure};
use rusqlite::{Connection, OptionalExtension, Transaction};
pub(super) fn complete(connection: &Connection, family: &str) -> Result<bool, StoreError> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM legacy_imports WHERE family = ?1)",
        [family],
        |row| row.get(0),
    )?)
}
pub(super) fn failures(connection: &Connection) -> Result<Vec<ImportFailure>, StoreError> {
    let mut query = connection.prepare(
        "SELECT family, error FROM legacy_imports WHERE error IS NOT NULL ORDER BY family",
    )?;
    Ok(query
        .query_map([], |row| {
            Ok(ImportFailure {
                record: row.get(0)?,
                reason: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}
pub(in super::super) fn readable(connection: &Connection, family: &str) -> Result<(), StoreError> {
    let error: Option<Option<String>> = connection
        .query_row(
            "SELECT error FROM legacy_imports WHERE family = ?1",
            [family],
            |row| row.get(0),
        )
        .optional()?;
    match error.flatten() {
        Some(reason) => Err(StoreError::InvalidState(format!(
            "pns: state error ({family} import: {reason}); legacy file retained"
        ))),
        None => Ok(()),
    }
}
pub(in super::super) fn replaced(
    transaction: &Transaction<'_>,
    family: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        "UPDATE legacy_imports SET error = NULL WHERE family = ?1",
        [family],
    )?;
    Ok(())
}
