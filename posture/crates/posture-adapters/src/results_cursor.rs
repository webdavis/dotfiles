//! Where the alerter recorded that it stopped, and the lock that keeps one run
//! at a time.
//!
//! THE CURSOR IS PUBLISHED BY RENAME, so a run killed mid-write leaves the old
//! position intact rather than a half-written one. A truncated cursor is not a
//! cursor that lost a digit: it is a cursor the domain refuses, which replays
//! the whole log and pages about it, and that is an expensive way to learn the
//! write was not atomic.
//!
//! THE LOCK IS NON-BLOCKING, which is the whole point of it. launchd fires the
//! alerter on a WatchPaths trigger, and a burst of writes fires it several
//! times over. A queued run would judge the same rows again the moment the
//! first finished; a refused one is a clean no-op.

use posture_application::{CursorStore, RunLock};
use posture_domain::{StoredCursor, parse_cursor, render_cursor};
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

pub struct CursorFile {
    path: PathBuf,
}

impl CursorFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl CursorStore for CursorFile {
    fn read(&self) -> Option<StoredCursor> {
        // ANYTHING UNREADABLE IS NO CURSOR, which the domain turns into a full
        // replay plus a page. Guessing at a damaged one is how a wrong offset
        // gets trusted and a batch is skipped in silence.
        parse_cursor(&fs::read_to_string(&self.path).ok()?)
    }

    fn write(&self, cursor: StoredCursor) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let mut pending = self.path.as_os_str().to_owned();
        pending.push(".tmp");
        let pending = PathBuf::from(pending);
        // A FAILED WRITE LEAVES THE OLD CURSOR, which re-reads rather than
        // skips. There is no path here that loses rows.
        if fs::write(&pending, format!("{}\n", render_cursor(cursor))).is_err() {
            return;
        }
        if fs::rename(&pending, &self.path).is_err() {
            let _ = fs::remove_file(&pending);
        }
    }
}

/// The single-instance lock every invocation contends on.
pub struct SingleRunLock {
    path: PathBuf,
    /// Held for the life of this struct, and released by the kernel on ANY
    /// exit, normal or not, so there is no stale lock to clean up.
    held: std::cell::RefCell<Option<File>>,
}

impl SingleRunLock {
    /// The lock sits beside the cursor it guards, so every invocation contends
    /// on one file whatever launched it.
    pub fn beside(cursor: &std::path::Path) -> Self {
        let mut path = cursor.as_os_str().to_owned();
        path.push(".lock");
        Self {
            path: path.into(),
            held: std::cell::RefCell::new(None),
        }
    }
}

impl RunLock for SingleRunLock {
    fn taken(&self) -> bool {
        if self.held.borrow().is_some() {
            return true;
        }
        let Some(parent) = self.path.parent() else {
            return false;
        };
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
        // O_CLOEXEC IS LOAD-BEARING. This process spawns children while it
        // holds the lock, and a child inheriting the descriptor would keep the
        // lock held after this run exits. The shell this replaces did the same
        // thing by hand, closing fd 9 on every single spawn.
        let Ok(file) = OpenOptions::new()
            .append(true)
            .create(true)
            .custom_flags(libc::O_CLOEXEC)
            .open(&self.path)
        else {
            // FAIL CLOSED. A setup error means this run does nothing, rather
            // than running unlocked beside a sibling that holds the lock.
            return false;
        };
        // NON-BLOCKING: a contended run is a no-op, never a queued one.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return false;
        }
        *self.held.borrow_mut() = Some(file);
        true
    }
}

#[cfg(test)]
#[path = "results_cursor/tests.rs"]
mod tests;
