//! The daemon's own stop, caught rather than taken.
//!
//! LAUNCHD STOPS THIS JOB WITH SIGTERM, whose default disposition ends the
//! process where it stands and leaves the failure page behind: a detached
//! child in a group of its own, reparented to pid 1, still holding the port
//! every later daemon's page then cannot bind. Catching the signal turns that
//! into a flag the loop reads on its next pass, which is where the page child
//! is stopped and waited for.
//!
//! ONE TICK OF LATENCY, and launchd's own SIGKILL a generous twenty seconds
//! behind it, so a loop that is wedged still goes down on schedule.

use std::sync::atomic::{AtomicBool, Ordering};

static STOPPING: AtomicBool = AtomicBool::new(false);

/// Catch SIGTERM for the rest of this process's life.
pub fn catch_termination() {
    // SAFE: `signal` takes an integer and a handler address, and `note_stop`
    // is an ordinary `extern "C"` function with static lifetime.
    unsafe { libc::signal(libc::SIGTERM, note_stop as *const () as libc::sighandler_t) };
}

/// Whether a stop has been asked for.
pub fn stopping() -> bool {
    STOPPING.load(Ordering::Relaxed)
}

/// ONE ATOMIC STORE AND NOTHING ELSE, which is all a signal handler may do:
/// allocating, locking or printing from here is undefined behaviour.
extern "C" fn note_stop(_signal: libc::c_int) {
    STOPPING.store(true, Ordering::Relaxed);
}
