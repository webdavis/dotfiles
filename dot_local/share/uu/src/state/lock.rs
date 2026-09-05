//! The run-wide lock: ONE RUN AT A TIME.
//!
//! Everything a run does reads, then writes, the marker and every lane's own
//! streak file with no other guard: two overlapping runs can both read the
//! same streak count and both write the same next value, which is the one
//! mechanism whose entire job is noticing a lane gone quiet, so a delayed or
//! duplicated staleness alert is the exact failure this exists to prevent.
//!
//! NON-BLOCKING, matching the two weekly jobs this ported from
//! (`/usr/bin/lockf -s -t 0`, kernel-backed and released automatically on exit
//! or a crash): a second run says so and exits rather than pretending it ran.

use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

fn path(home: &str) -> PathBuf {
    super::dir(home).join("run.lock")
}

/// The lock itself. Held only by virtue of the open file descriptor: the
/// kernel drops the `flock` the moment it closes, on a normal return or a
/// crash alike, so there is no stale-lock file to clean up by hand.
pub struct RunLock(#[allow(dead_code)] std::fs::File);

/// Why `acquire` could not hand back a lock: the ONE arm that is genuine
/// contention, and everything else. The call site says something different
/// for each, because "to avoid racing the run that already holds it" is only
/// true for `Contended`: a directory that could not be created or a lock file
/// that could not even be opened is an environment problem with its own real
/// cause, and this is the one place the operator hears about it, so blaming a
/// race that never happened would send them chasing the wrong thing.
pub enum LockFailure {
    /// `flock` itself refused: another run genuinely holds the lock right
    /// now.
    Contended(String),
    /// The lock file, or the directory it lives in, could not even be
    /// opened.
    Unavailable(String),
}

/// Take the run lock, or say why not. NON-BLOCKING (`LOCK_NB`): a second run
/// finding this one still going must say so and exit, never wait its turn
/// and then run stale.
pub fn acquire(home: &str) -> Result<RunLock, LockFailure> {
    let path = path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            LockFailure::Unavailable(format!("could not create {}: {error}", parent.display()))
        })?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            LockFailure::Unavailable(format!("could not open {}: {error}", path.display()))
        })?;
    // SAFETY: `file`'s descriptor is open and owned by this frame for the
    // whole call; `flock`'s only effect is the kernel's own lock table.
    let refused = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0;
    if refused {
        return Err(LockFailure::Contended(format!(
            "another run already holds {}",
            path.display()
        )));
    }
    Ok(RunLock(file))
}
