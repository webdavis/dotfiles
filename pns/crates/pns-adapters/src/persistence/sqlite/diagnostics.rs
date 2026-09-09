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
        self.write_diagnostic(&format!(
            "pns: state error ({operation}: {category}); recording failed"
        ));
    }

    pub fn report_delivery_health_failure(&self) {
        self.write_diagnostic(
            "pns: delivery health alarm failed; delivery pipeline needs attention",
        );
    }

    pub fn report_delivery_gap(&self) {
        self.write_diagnostic(
            "pns: delivery state unavailable; a delivery may not be queued or recorded",
        );
    }

    pub(super) fn delivery_recording_gap(&self) -> Result<bool, StoreError> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.log)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !file.metadata()?.is_file() {
            return Err(StoreError::UnsafeFile);
        }
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(65536)))?;
        let mut tail = Vec::new();
        file.take(65536).read_to_end(&mut tail)?;
        Ok(String::from_utf8_lossy(&tail).lines().any(|line| {
            line.starts_with("pns: delivery state unavailable;")
                || (line.starts_with("pns: state error (") && line.ends_with("; recording failed"))
        }))
    }

    fn write_diagnostic(&self, line: &str) {
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
        let _ = writeln!(log, "{line}");
    }
}
