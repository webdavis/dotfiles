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
///
/// THE NUMBER ITSELF IS `[storage] busy_deadline`'s default, and an install
/// that wants another one writes that key. It used to be overridable by an
/// environment variable that called itself test-only and was read here by
/// production code on every connection.
fn busy_timeout() -> Duration {
    static RESOLVED: std::sync::Mutex<Option<(String, Duration)>> = std::sync::Mutex::new(None);
    let home = std::env::var("HOME").unwrap_or_default();
    let mut cached = RESOLVED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((cached_home, deadline)) = cached.as_ref()
        && *cached_home == home
    {
        return *deadline;
    }
    let deadline = crate::install_settings(&home).busy_deadline;
    *cached = Some((home, deadline));
    deadline
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
            log: diagnostic_log(&state),
            state,
            claim: std::sync::Mutex::new(None),
            busy_timeout: busy_timeout(),
            legacy_records: false,
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
        prefer_wal(&connection)?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", true)?;
        migrations::migrate(&mut connection)?;
        Ok(connection)
    }
}

/// Put the database in WAL mode, TREATING A REFUSAL BY A BUSY DATABASE AS
/// SETTLED RATHER THAN FATAL.
///
/// MEASURED (sqlite 3.53.4, a 5 second `busy_timeout` set on the connection):
/// `PRAGMA journal_mode=WAL` answers SQLITE_BUSY in 0.000 seconds while
/// another connection holds the write lock on a rollback-journal database.
/// The conversion wants the database to itself and the busy handler is never
/// consulted for it, so no timeout covers this one statement.
///
/// THE COST OF TREATING IT AS FATAL WAS LOST RECORDS: it aborted the whole
/// open, and every record that open was about to write went with it, all
/// fail-quiet behind `report`. MEASURED over 3200 fresh opens raced 16 at a
/// time, 11 lost their open to this statement and none did with this function
/// in place. That is the dispatch write path dropping a decision, a journal
/// entry and a ledger row on ordinary contention, which is how five
/// concurrent events lost three of their five decision lines.
///
/// LOSING THE RACE IS NOT A DEGRADED MODE. The journal mode lives in the
/// database header, so the conversion is a one-time act that whichever
/// connection wins settles for every later one, and a connection that lost it
/// reads the header at its next transaction and uses the mode it finds there.
fn prefer_wal(connection: &Connection) -> Result<(), StoreError> {
    match connection.pragma_update(None, "journal_mode", "WAL") {
        Err(rusqlite::Error::SqliteFailure(failure, _))
            if matches!(
                failure.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) =>
        {
            Ok(())
        }
        other => other.map_err(StoreError::from),
    }
}

/// Where this store writes its diagnostics, derived from the state directory
/// it was handed.
///
/// The daemon's state tree is `<root>/.local/state/pns` and its diagnostics
/// belong in `<root>/.local/log/pns-daemon.log`, the same file its LaunchAgent
/// points both of its own streams at. A store rooted anywhere else keeps its
/// diagnostics inside that root, so the destination follows the state the
/// caller chose.
fn diagnostic_log(state: &Path) -> PathBuf {
    match state.parent().and_then(Path::parent) {
        Some(local) if state.ends_with(".local/state/pns") => local.join("log/pns-daemon.log"),
        _ => state.join("pns-daemon.log"),
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
