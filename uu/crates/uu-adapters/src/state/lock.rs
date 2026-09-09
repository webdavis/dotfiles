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

/// The run owns the lock until its guard drops. A duplicate descriptor may
/// outlive the run, so closing this file alone need not release the lock.
pub struct RunLock(std::fs::File);

impl Drop for RunLock {
    fn drop(&mut self) {
        // SAFETY: this guard still owns the open descriptor. Explicitly unlocking
        // releases its shared flock before any inherited descriptor closes.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

use uu_application::LockFailure;

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

#[cfg(test)]
mod tests;
