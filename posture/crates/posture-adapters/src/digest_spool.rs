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

use posture_application::{ClaimFailure, ClaimedBatch, DigestRow, DigestSpool};
use std::fs;
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
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
        // ONE HANDLE ANSWERS BOTH QUESTIONS. Reading the tail through a second
        // open left a window: a claim could rename the spool away between the
        // two opens, and the separator would then describe a file the write
        // never reached, opening a fresh spool with a blank first line.
        // `O_APPEND` ignores the read position, so seeking to read the tail
        // cannot move where the write lands.
        let Ok(mut spool) = fs::OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .mode(0o600)
            .open(onto)
        else {
            return;
        };
        // A concurrent append can only add whole lines after that tail, so the
        // answer still holds when the write lands.
        let Ok(separator) = owes_separator(&mut spool) else {
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

/// Whether the spool's last line is still open, read through the same handle
/// the append will use. An empty spool owes nothing.
fn owes_separator(spool: &mut fs::File) -> std::io::Result<bool> {
    let length = spool.metadata()?.len();
    if length == 0 {
        return Ok(false);
    }
    spool.seek(SeekFrom::Start(length - 1))?;
    let mut last = [0u8; 1];
    spool.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// 0600, which is what a file holding full filesystem paths gets.
fn owner_only(path: &Path) -> std::io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

/// A claim the filesystem refused, named for whoever reads the log.
fn unclaimable(path: &Path, error: &std::io::Error) -> ClaimFailure {
    ClaimFailure(format!("{}: {error}", path.display()))
}

/// The text a batch can still be read from, and how many lines arrived in all.
///
/// A TORN LINE IS EXPECTED, NOT EXCEPTIONAL. The alerter appends from another
/// process, so an interrupted write leaves bytes that are not a record and may
/// not even be UTF-8. Such a line is DROPPED here rather than failing the read,
/// because a failed read puts the whole batch back to fail again on every later
/// run while the spool grows behind it. It still counts toward what arrived,
/// which is what makes the drop visible instead of silent.
fn readable_lines(bytes: &[u8]) -> (usize, String) {
    let mut arrived = 0;
    let mut readable = String::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        match std::str::from_utf8(line) {
            // WHITESPACE IS NOT A FINDING, so a blank line is neither counted
            // nor carried.
            Ok(text) if text.trim().is_empty() => {}
            Ok(text) => {
                arrived += 1;
                readable.push_str(text);
                readable.push('\n');
            }
            Err(_) => arrived += 1,
        }
    }
    (arrived, readable)
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

    fn claim(&self) -> Result<Option<ClaimedBatch>, ClaimFailure> {
        let metadata = match fs::metadata(&self.store) {
            Ok(metadata) => metadata,
            // ABSENT AND EMPTY ARE THE SAME ANSWER: there is nothing to
            // summarize. Every other stat failure is a real one and says so,
            // because a broken day that reads as a quiet day is a day nobody
            // hears about.
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(unclaimable(&self.store, &error)),
        };
        if metadata.len() == 0 {
            return Ok(None);
        }
        let claimed = self.claim_path();
        // A FAILED RENAME LEAVES THE SPOOL UNTOUCHED, so nothing is lost and
        // the next run retries the same bytes.
        if let Err(error) = fs::rename(&self.store, &claimed) {
            return Err(unclaimable(&self.store, &error));
        }
        // An unreadable claim stays intact for the next orphan sweep.
        let bytes = match fs::read(&claimed) {
            Ok(bytes) => bytes,
            Err(error) => return Err(unclaimable(&claimed, &error)),
        };
        let (item_count, readable) = readable_lines(&bytes);
        // A spool holding only blank lines has nothing in it, and claiming it
        // is how those lines get cleared.
        if item_count == 0 {
            let _ = fs::remove_file(&claimed);
            return Ok(None);
        }
        let rows = posture_protocol::decode_spool(&readable)
            .into_iter()
            .map(|record| DigestRow {
                detector: record.detector,
                identity: record.identity,
                summary: record.summary,
            })
            .collect();
        Ok(Some(ClaimedBatch {
            handle: claimed.to_string_lossy().into_owned(),
            item_count,
            rows,
        }))
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
