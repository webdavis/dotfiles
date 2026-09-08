use super::{SqliteStore, StoreError};
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

impl SqliteStore {
    pub(super) fn report(&self, operation: &'static str, error: &StoreError) {
        let category = match error {
            StoreError::Database(rusqlite::Error::SqliteFailure(error, _)) => match error.code {
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                    "database busy"
                }
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                    "database corrupt"
                }
                _ => "database refused the operation",
            },
            StoreError::Database(_) => "database refused the operation",
            StoreError::Io(_) => "state file unavailable",
            StoreError::InvalidState(_) => "unreadable state record",
            StoreError::UnsafeFile => "state file is not private and regular",
            StoreError::UnsupportedSchema(_) => "unsupported database schema",
        };
        // Hook streams never receive this diagnostic. Failure to write the
        // existing daemon log cannot change the destination's outcome either.
        let Some(parent) = self.log.parent() else {
            return;
        };
        if DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .is_err()
        {
            return;
        }
        let Ok(mut log) = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.log)
        else {
            return;
        };
        if !log.metadata().is_ok_and(|metadata| metadata.is_file()) {
            return;
        }
        let _ = writeln!(
            log,
            "pns: state error ({operation}: {category}); recording failed"
        );
    }
}
