use posture_application::QueueHealth;
use posture_domain::QueueCounts;
use rusqlite::{Connection, OpenFlags};
use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct QueueDatabase {
    path: PathBuf,
    pns: bool,
    timeout: Duration,
}
impl QueueDatabase {
    pub fn legacy(path: PathBuf) -> Self {
        Self {
            path,
            pns: false,
            timeout: Duration::from_millis(200),
        }
    }
    pub fn pns(path: PathBuf) -> Self {
        Self {
            path,
            pns: true,
            timeout: Duration::from_millis(200),
        }
    }
    fn open(&self) -> rusqlite::Result<Connection> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(self.timeout)?;
        let deadline = Instant::now() + self.timeout;
        connection.progress_handler(1000, Some(move || Instant::now() >= deadline))?;
        Ok(connection)
    }
    fn read(&self) -> Result<QueueCounts, rusqlite::Error> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        if self.pns {
            // Read the deployed ledger contract, never invoke pns or migrate its store.
            // A future schema must be reviewed before this independent reader accepts it.
            let version: u32 =
                transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
            if version != 8 {
                return Err(rusqlite::Error::InvalidQuery);
            }
            transaction.query_row("SELECT COUNT(CASE WHEN acknowledged = 0 AND deadlettered_at IS NULL THEN 1 END), COUNT(deadlettered_at) FROM ledger_legs", [], |row| Ok(QueueCounts { pending: Some(row.get(0)?), deadletters: Some(row.get(1)?) }))
        } else {
            Ok(QueueCounts {
                pending: lazy_count(
                    &transaction,
                    "pending_alerts",
                    "SELECT COUNT(*) FROM pending_alerts",
                )
                .ok(),
                deadletters: lazy_count(
                    &transaction,
                    "dead_letter_alerts",
                    "SELECT COUNT(*) FROM dead_letter_alerts",
                )
                .ok(),
            })
        }
    }
    fn files_safe(&self) -> bool {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.path.as_os_str().to_owned();
            path.push(suffix);
            match fs::symlink_metadata(PathBuf::from(path)) {
                Ok(metadata)
                    if metadata.is_file()
                        && (!self.pns || metadata.permissions().mode() & 0o077 == 0) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                _ => return false,
            }
        }
        true
    }
}
impl QueueHealth for QueueDatabase {
    fn counts(&mut self) -> QueueCounts {
        let unreadable = QueueCounts {
            pending: None,
            deadletters: None,
        };
        if !self.files_safe() {
            return unreadable;
        }
        if !self.pns
            && fs::symlink_metadata(&self.path)
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        {
            return QueueCounts {
                pending: Some(0),
                deadletters: Some(0),
            };
        }
        self.read().unwrap_or(unreadable)
    }
}
fn lazy_count(connection: &Connection, table: &str, sql: &str) -> rusqlite::Result<u64> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = ?1)",
        [table],
        |row| row.get(0),
    )?;
    if exists {
        connection.query_row(sql, [], |row| row.get(0))
    } else {
        Ok(0)
    }
}
#[cfg(test)]
mod tests;
