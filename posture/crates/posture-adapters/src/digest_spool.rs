//! The digest spool as a file, and the moves that hand a batch between runs.
//!
//! TWO PROCESSES SHARE THIS FILE and never speak to each other: the alerter
//! appends a line per non-paging finding, and once a day the digest drains it.
//! The whole protocol is `rename`, which is atomic within a directory, so the
//! alerter's next append always lands in a live spool and never in a batch
//! somebody is already reading.
//!
//! MODES ARE PART OF THE CONTRACT. The spool carries full filesystem paths,
//! which is what makes it useful for triage and also what makes it private, so
//! the directory is 0700 and every file in it is 0600. The directory mode is
//! set BEFORE any file exists, so even the window before a file's own mode is
//! applied sits under an unreadable parent.

use posture_application::{ClaimedBatch, DigestRow, DigestSpool};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// What a claimed batch is named while this run owns it.
const CLAIM_SUFFIX: &str = ".build";

/// Where a delivered batch is kept for forensics.
const KEEP_SUFFIX: &str = ".last";

pub struct DigestSpoolFile {
    store: PathBuf,
    /// What a claim names itself, unique per invocation.
    ///
    /// THE PROCESS ID IS LOAD-BEARING, not decoration. A timestamp alone is
    /// only unique per second, so two runs starting in the same second would
    /// claim the same path and the second would clobber the first.
    stamp: String,
}

impl DigestSpoolFile {
    pub fn new(store: PathBuf, seconds: u64, pid: u32) -> Self {
        Self {
            store,
            stamp: format!("{seconds}.{pid}"),
        }
    }

    fn claim_path(&self) -> PathBuf {
        self.sibling(&format!(".{}{CLAIM_SUFFIX}", self.stamp))
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut path = self.store.as_os_str().to_owned();
        path.push(suffix);
        path.into()
    }

    /// Append `from` onto `onto`, then remove `from` only if that worked.
    ///
    /// APPEND RATHER THAN RENAME OR REWRITE, always. The alerter can add a
    /// finding to the fresh spool while this run is building: a rename would
    /// destroy that concurrent append, and a read-then-rewrite would lose any
    /// line that landed between the read and the write. An `O_APPEND` write
    /// lands after whatever is there when it runs. Order within a grouped
    /// digest carries no meaning, so appending costs nothing and cannot clobber.
    fn fold(from: &Path, onto: &Path) {
        let Ok(bytes) = fs::read(from) else { return };
        // Whether a separator is owed is read off the tail before the write.
        // A concurrent append can only add whole lines after that tail, so the
        // answer still holds when the write lands.
        let separator = match last_byte(onto) {
            Ok(last) => last.is_some_and(|byte| byte != b'\n'),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => return,
        };
        let Ok(mut spool) = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(onto)
        else {
            return;
        };
        let mut payload = Vec::with_capacity(bytes.len() + 1);
        if separator {
            payload.push(b'\n');
        }
        payload.extend_from_slice(&bytes);
        if spool.write_all(&payload).is_ok() {
            let _ = owner_only(onto);
            let _ = fs::remove_file(from);
        }
    }
}

/// The file's last byte, or `None` for an empty file.
fn last_byte(path: &Path) -> std::io::Result<Option<u8>> {
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    if length == 0 {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(length - 1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    Ok(Some(last[0]))
}

/// 0600, which is what a file holding full filesystem paths gets.
fn owner_only(path: &Path) -> std::io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

impl DigestSpool for DigestSpoolFile {
    fn sweep_orphans(&self) {
        let Some(directory) = self.store.parent() else {
            return;
        };
        let Some(prefix) = self.store.file_name().and_then(|name| name.to_str()) else {
            return;
        };
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            // A STEADY STATE SWEEPS NOTHING: a run that finished left no claim
            // behind, because it either kept the batch or restored it.
            if name.starts_with(prefix) && name.ends_with(CLAIM_SUFFIX) {
                Self::fold(&path, &self.store);
            }
        }
    }

    fn claim(&self) -> Option<ClaimedBatch> {
        let metadata = fs::metadata(&self.store).ok()?;
        // ABSENT AND EMPTY ARE THE SAME ANSWER: there is nothing to summarize.
        if metadata.len() == 0 {
            return None;
        }
        let claimed = self.claim_path();
        // A FAILED RENAME LEAVES THE SPOOL UNTOUCHED, so nothing is lost and
        // the next run retries the same bytes.
        fs::rename(&self.store, &claimed).ok()?;
        // An unreadable claim stays intact for the next orphan sweep.
        let contents = fs::read_to_string(&claimed).ok()?;
        // WHITESPACE IS NOT A FINDING. A spool holding only blank lines has
        // nothing in it, and claiming it is how those lines get cleared.
        if contents.trim().is_empty() {
            let _ = fs::remove_file(&claimed);
            return None;
        }
        let item_count = contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        let rows = posture_protocol::decode_spool(&contents)
            .into_iter()
            .map(|record| DigestRow {
                detector: record.detector,
                identity: record.identity,
                summary: record.summary,
            })
            .collect();
        Some(ClaimedBatch {
            handle: claimed.to_string_lossy().into_owned(),
            item_count,
            rows,
        })
    }

    fn keep(&self, batch: &ClaimedBatch) {
        let kept = self.sibling(KEEP_SUFFIX);
        // A FAILED SAME-DIRECTORY RENAME MEANS A BROKEN FILESYSTEM, and the
        // findings also live in the results log, so a lost forensic copy is
        // cheap. Removing the claim is what stops the next run's sweep folding
        // an already-delivered batch back in and sending it twice.
        if fs::rename(&batch.handle, &kept).is_err() {
            let _ = fs::remove_file(&batch.handle);
            return;
        }
        let _ = owner_only(&kept);
    }

    fn restore(&self, batch: &ClaimedBatch) {
        Self::fold(Path::new(&batch.handle), &self.store);
    }
}

/// Make the spool's directory, unreadable to anyone else, before a file exists.
pub fn prepare_spool_directory(store: &Path) -> std::io::Result<()> {
    let Some(directory) = store.parent() else {
        return Ok(());
    };
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
}

#[cfg(test)]
#[path = "digest_spool/tests.rs"]
mod tests;
