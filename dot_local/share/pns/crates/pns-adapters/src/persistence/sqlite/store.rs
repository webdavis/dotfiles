use super::{StoreError, migrations};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

// Match the prior ring lock's 200 attempts spaced one millisecond apart.
// SQLite owns this bounded wait; refused delivery writes still report and continue.
const BUSY_TIMEOUT: Duration = Duration::from_millis(200);

pub struct SqliteStore {
    pub(super) state: PathBuf,
    pub(super) claim: std::sync::Mutex<Option<i64>>,
    pub(super) busy_timeout: Duration,
    pub(super) log: PathBuf,
    legacy_records: bool,
}
impl SqliteStore {
    // Legacy consumers cannot see an empty database before their import commits.
    pub fn for_records(state: PathBuf) -> Self {
        let mut store = Self::new(state);
        store.legacy_records = true;
        store
    }

    // Ledger-only composition has no legacy rows to import.
    pub fn new(state: PathBuf) -> Self {
        Self {
            state,
            claim: std::sync::Mutex::new(None),
            busy_timeout: BUSY_TIMEOUT,
            legacy_records: false,
            log: PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/log/pns-daemon.log"),
        }
    }

    pub(crate) fn transaction<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let value = operation(&transaction)?;
        transaction.commit()?;
        Ok(value)
    }

    pub(crate) fn connect(&self) -> Result<Connection, StoreError> {
        let mut connection = self.open()?;
        if self.legacy_records {
            self.ensure_imported(&mut connection)?;
        }
        Ok(connection)
    }

    pub(super) fn read_only(&self) -> Result<Connection, StoreError> {
        for name in ["pns.db", "pns.db-wal", "pns.db-shm", "pns.db-journal"] {
            private_regular_file(&self.state.join(name))?;
        }
        let connection = Connection::open_with_flags(
            self.state.join("pns.db"),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(self.busy_timeout)?;
        let version = migrations::validate(&connection)?;
        if version != migrations::VERSION {
            return Err(StoreError::UnsupportedSchema(version));
        }
        Ok(connection)
    }

    pub(super) fn open(&self) -> Result<Connection, StoreError> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.state)?;
        let path = self.state.join("pns.db");
        let created = create_database(&path)?;
        let connection = self.open_existing()?;
        if created {
            fs::File::open(&self.state)?.sync_all()?;
        }
        Ok(connection)
    }
    pub(super) fn existing_transaction<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut connection = self.open_existing()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let value = operation(&transaction)?;
        transaction.commit()?;
        Ok(value)
    }
    fn open_existing(&self) -> Result<Connection, StoreError> {
        let path = self.state.join("pns.db");
        for name in ["pns.db", "pns.db-wal", "pns.db-shm", "pns.db-journal"] {
            private_regular_file(&self.state.join(name))?;
        }
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(self.busy_timeout)?;
        migrations::validate(&connection)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", true)?;
        migrations::migrate(&mut connection)?;
        Ok(connection)
    }
}

fn create_database(path: &Path) -> Result<bool, StoreError> {
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
    {
        Ok(file) => {
            file.sync_all()?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn private_regular_file(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.permissions().mode() & 0o077 == 0 => Ok(()),
        Ok(_) => Err(StoreError::UnsafeFile),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
