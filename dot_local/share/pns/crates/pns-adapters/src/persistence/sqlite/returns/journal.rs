use super::StoreError;
use pns_domain::missed::Entry;
use rusqlite::Transaction;

pub(super) fn claim(
    transaction: &Transaction<'_>,
) -> Result<(Vec<Entry>, Option<i64>), StoreError> {
    let mut owners = transaction.prepare("SELECT id, owner FROM return_claims ORDER BY id")?;
    let owners = owners
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u32>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let abandoned = owners
        .into_iter()
        .filter_map(|(id, owner)| {
            crate::marker_files::owner_is_gone(&owner.to_string()).then_some(id)
        })
        .collect::<Vec<_>>();
    let pending: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM journal WHERE claim IS NULL)",
        [],
        |row| row.get(0),
    )?;
    if !pending && abandoned.is_empty() {
        return Ok((Vec::new(), None));
    }
    transaction.execute(
        "INSERT INTO return_claims(owner) VALUES (?1)",
        [std::process::id()],
    )?;
    let owned = transaction.last_insert_rowid();
    transaction.execute("UPDATE journal SET claim = ?1 WHERE claim IS NULL", [owned])?;
    for abandoned in abandoned {
        transaction.execute(
            "UPDATE journal SET claim = ?1 WHERE claim = ?2",
            [owned, abandoned],
        )?;
        transaction.execute("DELETE FROM return_claims WHERE id = ?1", [abandoned])?;
    }
    let mut rows = transaction.prepare("SELECT line FROM journal WHERE claim = ?1 ORDER BY seq")?;
    let contents = rows
        .query_map([owned], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .concat();
    Ok((crate::journal_codec::entries(&contents), Some(owned)))
}
