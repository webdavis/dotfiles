//! The one poller's claim on the bridge, arbitrated by the kernel rather than
//! by a pathname.
//!
//! NOT THE NAMED-LOCK PROTOCOL the rest of this binary uses, and the
//! difference is the whole module. That one arbitrates an exclusive CREATE and
//! believes a holder until the file's own mtime ages out, so a process that
//! died leaves a file somebody has to reclaim. Reclamation is a second race
//! run on pathnames: one reclaimer renames the dead lock away and recreates
//! it, and a second reclaimer arriving a moment later renames the FRESH lock
//! away and recreates it too, so both are inside the poll and the older
//! reading can still be the last one written. Ageing the file out is also the
//! only way a killed poller is ever cleared, and a killed poller is the
//! ordinary end of one: the daemon kills every child that outlives its bound,
//! and a killed process runs no `Drop`.
//!
//! AN INHERITED DESCRIPTOR HOLDS IT TOO, which is the one property of a
//! kernel lock that surprises: `fork` duplicates the open file description, so
//! a child holds the same lock until its `exec` closes the descriptor
//! (measured at one 5ms tick). Nothing is spawned between the claim and the
//! publish, so a poll never opens that window; the TEST binary does, because
//! other tests in it spawn subprocesses while this one holds a lock, which is
//! why the test that relocks immediately allows for it.
//!
//! A KERNEL LOCK HAS NEITHER PROBLEM. The lock belongs to the open file
//! description, so the last close releases it, INCLUDING the close the kernel
//! does for a process it killed. There is no stale window to tune, no
//! reclamation to arbitrate and nothing to unlink: the file stays at its name
//! for good and only the lock on it moves. Two racers cannot both be told they
//! succeeded, whenever they arrive.

use std::fs::File;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The name beside the presence state file that arbitrates between two
/// pollers. It is created once and never removed: the lock is on the open
/// file, not on the name.
pub const LOCK_FILE: &str = "presence-poll.lock";

/// Owner-only, as every other file under the state directory is.
const LOCK_MODE: u32 = 0o600;

/// Who owns the poll, as the three answers a caller acts on differently.
pub enum Claim {
    /// This process does, for as long as the handle lives. Dropping it gives
    /// the lock back and so does dying, which is why nothing here has a
    /// release path of its own.
    Held(File),
    /// Somebody else is inside a poll right now. SAID OUT LOUD by a caller
    /// with a human behind it: a hand-typed poll that read no bridge and
    /// published nothing looks exactly like one that worked.
    Busy,
    /// The lock file could not be opened at all, which is an unwritable state
    /// directory or a symlink dropped at the name. QUIET, because the publish
    /// this guards would fail for the same reason and the daemon runs this
    /// every few seconds: a complaint would be a line a second for as long as
    /// the condition lasted.
    Unavailable,
}

/// The poll claimed for as long as the returned handle lives.
///
/// IT NEVER FOLLOWS A LINK (`O_NOFOLLOW`), keeping the property the exclusive
/// create had for free: a symlink dropped at this name cannot move the lock,
/// or the mode it is created with, to a file outside the state directory.
///
/// AND IT NEVER WAITS (`O_NONBLOCK`), which is the same rule one step further
/// out. A write-only open of a FIFO nobody is reading BLOCKS, so a named pipe
/// standing at this name would hang a hand-typed poll outright and burn every
/// daemon poll's whole child bound while the reading it never refreshed aged
/// out. The flag turns that open into `ENXIO`, and the file-type check below
/// catches the same pipe with a reader on the other end, which opens fine and
/// is still not a lock. A regular file is unaffected by the flag: it has no
/// blocking open to skip.
///
/// A LOCK IS A REGULAR FILE OR IT IS NOTHING, and the type is read off the
/// HANDLE rather than off the path, so nothing swapped in between the two
/// answers a question about a different file.
pub fn claim(lock: &Path) -> Claim {
    let Ok(file) = File::options()
        .create(true)
        .write(true)
        .mode(LOCK_MODE)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(lock)
    else {
        return Claim::Unavailable;
    };
    if !file.metadata().is_ok_and(|found| found.is_file()) {
        return Claim::Unavailable;
    }
    match file.try_lock() {
        Ok(()) => Claim::Held(file),
        Err(std::fs::TryLockError::WouldBlock) => Claim::Busy,
        Err(std::fs::TryLockError::Error(_)) => Claim::Unavailable,
    }
}

#[cfg(test)]
mod tests;
