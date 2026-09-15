use super::{StoreError, migrations};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a writer waits for the database's write lock before the operation
/// is refused.
///
/// THIS NUMBER BOUNDS A WEDGED WRITER AND MEASURES NOTHING. Every transaction
/// in this crate is a closure of short statements with no delivery, no network
/// and no sleep inside it, and a writer that dies drops its locks with its
/// process, so nothing legitimate holds the write lock for whole seconds.
/// A wait that expires is therefore a machine in trouble, not a busy one.
///
/// IT USED TO MEASURE CONTENTION, at the prior ring lock's 200 milliseconds,
/// and MEASURED silently lost records for it: five events firing together
/// (a Stop hook, the long-running notifier and their siblings are an ordinary
/// pair on a busy machine) exhausted that wait on a loaded runner and the
/// refusal is fail-quiet, so the decision simply never appeared in the log the
/// operator opens to ask why. Five seconds is the span this tool already uses
/// for "a holder this long is broken rather than busy" (`RING_LOCK_STALE_SECS`),
/// and contention between short transactions clears orders of magnitude below it.
///
/// THE WORST CASE IS AN INTERACTIVE HOOK STALLING FOR SECONDS, not milliseconds:
/// `record_decision` runs in-process before the rest of the event path
/// (`event_flow.rs`), so a genuinely wedged writer now costs a Stop or prompt
/// hook up to this whole bound per lock acquisition, and the event path makes
/// more than one. That is the trade this number makes: a rare multi-second
/// stall against the common case this change fixes, records silently lost to
/// ordinary contention.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// A TEST-ONLY OVERRIDE of that bound, in `PNS_RING_LOCK_TEST_DELAY_MS`'s own
/// style and for its mirror-image reason: a test that STAGES a wedged writer
/// and asserts the refusal would otherwise spend the whole product bound
/// waiting for a lock it deliberately holds itself. Unset in every real
/// invocation, which is the only way the shipped default is ever used.
const BUSY_TIMEOUT_OVERRIDE: &str = "PNS_DB_BUSY_TIMEOUT_MS";

fn busy_timeout() -> Duration {
    std::env::var(BUSY_TIMEOUT_OVERRIDE)
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(BUSY_TIMEOUT, Duration::from_millis)
}

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
            busy_timeout: busy_timeout(),
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
