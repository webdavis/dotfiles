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
pub fn claim(lock: &Path) -> Claim {
    let Ok(file) = File::options()
        .create(true)
        .write(true)
        .mode(LOCK_MODE)
        .custom_flags(libc::O_NOFOLLOW)
        .open(lock)
    else {
        return Claim::Unavailable;
    };
    match file.try_lock() {
        Ok(()) => Claim::Held(file),
        Err(std::fs::TryLockError::WouldBlock) => Claim::Busy,
        Err(std::fs::TryLockError::Error(_)) => Claim::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::{Claim, LOCK_FILE, claim};
    use std::path::PathBuf;

    /// A scratch state directory of this test's own, named so two tests and
    /// two runs of one test never share a file.
    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "pns-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        directory
    }

    #[test]
    fn two_live_contenders_and_exactly_one_is_inside_the_poll() {
        // THE WHOLE POINT OF THE LOCK: two pollers reaching the bridge at once
        // publish in the order they FINISH, so the stalled one's older reading
        // lands last and `classify` reads it as current.
        let lock = scratch("presence-lock-contenders").join(LOCK_FILE);
        let first = claim(&lock);
        let second = claim(&lock);
        assert!(
            matches!(first, Claim::Held(_)),
            "the first claim stood down"
        );
        assert!(
            matches!(second, Claim::Busy),
            "a second poller was let inside a poll somebody else was holding"
        );
    }

    #[test]
    fn a_claim_given_back_can_be_taken_again() {
        // A LOCK RATHER THAN A LATCH: a hold left behind would stand every
        // later poll down, which is a sensor that answers once and goes quiet.
        //
        // RETRIED FOR A MOMENT, for the inherited descriptor this module's
        // header names: another test in this binary spawning a subprocess
        // while this one holds the lock leaves a child holding it too until
        // its `exec`. A lock that is never given back still fails here, on
        // the deadline.
        let lock = scratch("presence-lock-again").join(LOCK_FILE);
        drop(claim(&lock));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut taken = claim(&lock);
        while matches!(taken, Claim::Busy) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
            taken = claim(&lock);
        }
        assert!(matches!(taken, Claim::Held(_)));
    }

    #[test]
    fn the_poll_a_killed_holder_was_inside_is_claimable_at_once() {
        // TWO REAL PROCESSES, because that is the only place this behavior
        // exists: the holder is SIGKILLed, so it runs no release of its own,
        // and the next poller must be inside the poll immediately rather than
        // waiting out a window measured off a file's clock.
        let lock = scratch("presence-lock-killed").join(LOCK_FILE);
        // The child opens this by path with no `O_CREAT`, whose mode argument
        // rides a variadic tail this cannot fill, so the file exists first.
        std::fs::write(&lock, b"").expect("the lock file");
        let path =
            std::ffi::CString::new(lock.to_str().expect("a utf-8 path")).expect("no interior nul");
        let mut ready = [0; 2];
        assert_eq!(unsafe { libc::pipe(ready.as_mut_ptr()) }, 0, "the pipe");

        // SAFETY: the child calls nothing but async-signal-safe libc, which is
        // all a fork of a threaded test binary may call: it takes the lock on
        // a fresh open file description, says so down the pipe, and parks.
        // `alarm` is the backstop that reaps it if this test dies first.
        let child = unsafe { libc::fork() };
        assert!(child >= 0, "fork");
        if child == 0 {
            unsafe {
                // EVERY INHERITED DESCRIPTOR CLOSED FIRST, the pipe this
                // answers on excepted. `fork` duplicates the open file
                // descriptions of every thread in this binary, so a lock
                // another test was holding at this instant would be held by
                // this child too, and this child lives until the kill below
                // rather than for the moment an `exec` takes.
                let top = libc::getdtablesize();
                for descriptor in 3..top {
                    if descriptor != ready[1] {
                        libc::close(descriptor);
                    }
                }
                libc::alarm(5);
                let descriptor = libc::open(path.as_ptr(), libc::O_RDWR);
                if descriptor < 0 || libc::flock(descriptor, libc::LOCK_EX | libc::LOCK_NB) != 0 {
                    libc::_exit(1);
                }
                libc::write(ready[1], b"h".as_ptr().cast(), 1);
                loop {
                    libc::pause();
                }
            }
        }
        unsafe { libc::close(ready[1]) };
        let mut said = [0_u8; 1];
        let read = unsafe { libc::read(ready[0], said.as_mut_ptr().cast(), 1) };
        assert_eq!(read, 1, "the child never took the lock");

        assert!(
            matches!(claim(&lock), Claim::Busy),
            "a live holder in another process did not stand this poll down"
        );

        // KILLED AND REAPED, in that order: the wait is what makes the child's
        // last close a fact rather than a race with this assertion.
        assert_eq!(unsafe { libc::kill(child, libc::SIGKILL) }, 0, "the kill");
        let mut status = 0;
        assert_eq!(
            unsafe { libc::waitpid(child, &mut status, 0) },
            child,
            "the reap"
        );
        assert!(
            matches!(claim(&lock), Claim::Held(_)),
            "the poll a killed holder was inside could not be claimed"
        );
    }

    #[test]
    fn a_symlink_at_the_lock_name_is_refused_rather_than_followed() {
        // The property the exclusive create had, kept: a link dropped at this
        // name must not move the lock, or its mode, out of the state
        // directory.
        let state = scratch("presence-lock-symlink");
        let elsewhere = state.join("elsewhere");
        std::fs::write(&elsewhere, b"").expect("the target");
        std::os::unix::fs::symlink(&elsewhere, state.join(LOCK_FILE)).expect("the link");
        assert!(matches!(claim(&state.join(LOCK_FILE)), Claim::Unavailable));
    }

    #[test]
    fn a_lock_that_cannot_be_opened_is_unavailable_rather_than_held() {
        // An unwritable state directory publishes nothing either way, so this
        // is the quiet refusal rather than the loud one.
        let state = scratch("presence-lock-unwritable");
        assert!(matches!(
            claim(&state.join("no-such-directory").join(LOCK_FILE)),
            Claim::Unavailable
        ));
    }
}
