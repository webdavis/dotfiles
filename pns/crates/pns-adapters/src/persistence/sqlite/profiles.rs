//! The profile's own durable state: the operator's override.
//!
//! THE OVERRIDE IS ONE ROW ON THE SCALAR MODEL the quiet expiry already uses,
//! for that row's own reason: a name and an expiry in two rows are two values
//! that can disagree about whether an override is standing.

use super::{SqliteStore, StoreError, scalar::Scalar};
use pns_domain::profiles::{Override, format_override, parse_override};
use rusqlite::Transaction;

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE profile_override (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);",
    )?;
    Ok(())
}

impl SqliteStore {
    /// The standing override, or None where there is none and where the body
    /// could not be read.
    ///
    /// AN UNREADABLE BODY IS NO OVERRIDE. The rules then decide, which is the
    /// fail-open direction every other reading here takes: not knowing costs
    /// the narrowing, never the notification.
    pub fn profile_override(&self) -> Result<Option<Override>, StoreError> {
        Ok(Scalar::Profile
            .stored(&self.connect()?)?
            .as_deref()
            .and_then(parse_override))
    }

    /// Set the override, or clear it with `None`.
    pub fn set_profile_override(&self, standing: Option<&Override>) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            Scalar::Profile.write(transaction, standing.map(format_override).as_deref())
        })
    }

    #[cfg(test)]
    pub(super) fn write_profile_override_body(&self, body: &str) -> Result<(), StoreError> {
        self.transaction(|transaction| Scalar::Profile.write(transaction, Some(body)))
    }
}
