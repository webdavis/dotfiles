use super::{SqliteStore, StoreError};
use rusqlite::{Connection, Transaction, TransactionBehavior};
mod claims;
mod families;
pub(super) use claims::inspect;
use families::FAMILIES;
mod read;
mod records;
mod status;
mod validate;
pub(super) use status::{readable, replaced};

pub use pns_application::ImportFailure;
impl SqliteStore {
    pub fn import_legacy(&self) -> Result<Vec<ImportFailure>, StoreError> {
        self.import_connection(&mut self.open()?)
    }
    pub(super) fn ensure_imported(&self, connection: &mut Connection) -> Result<(), StoreError> {
        let complete = FAMILIES.into_iter().try_fold(true, |all, family| {
            status::complete(connection, family.name()).map(|done| all && done)
        })?;
        if !complete {
            self.import_connection(connection)?;
        }
        Ok(())
    }
    fn import_connection(
        &self,
        connection: &mut Connection,
    ) -> Result<Vec<ImportFailure>, StoreError> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let failures = import(&transaction, &self.state)?;
        transaction.commit()?;
        for family in FAMILIES {
            if failures
                .iter()
                .any(|failure| failure.record == family.name())
            {
                self.report(
                    family.name(),
                    &StoreError::InvalidState("legacy import failed".into()),
                );
            }
        }
        Ok(failures)
    }
    pub fn import_failures(&self) -> Result<Vec<ImportFailure>, StoreError> {
        status::failures(&self.connect()?)
    }
}
pub(super) fn import(
    transaction: &Transaction<'_>,
    state: &std::path::Path,
) -> Result<Vec<ImportFailure>, StoreError> {
    let remaining = FAMILIES
        .into_iter()
        .map(|family| {
            status::complete(transaction, family.name()).map(|done| (!done).then_some(family))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if remaining.is_empty() {
        return status::failures(transaction);
    }
    let claims = inspect(state, crate::now_secs())?;
    for family in remaining {
        let held_error = if matches!(family, families::Family::Ring(super::rows::Ring::Journal)) {
            claims::import_journal(transaction, &claims)?
        } else {
            None
        };
        let error = match read::read(&state.join(family.name()), family.limit()) {
            Ok(body) => {
                records::put(transaction, family, &body)?;
                validate::error(family, &body)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => Some("legacy record could not be read"),
        };
        let window_error = if matches!(family, families::Family::Return) {
            claims::import_window(transaction, &claims)?
        } else {
            None
        };
        let error = error.or(held_error).or(window_error);
        transaction.execute(
            "INSERT INTO legacy_imports(family, error) VALUES (?1, ?2)",
            rusqlite::params![family.name(), error],
        )?;
    }
    status::failures(transaction)
}
