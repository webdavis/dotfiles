//! Waiting one child out, and the group kill that enforces its deadline.
//!
//! THE DEADLINE COVERS THE OUTPUT READ, not only the child's own lifetime.
//! The hang this exists for is a child that EXITS while something it left
//! behind (a backgrounded process, a detached daemon) still holds its stdout
//! or stderr: the wait answers at once and the read blocks forever. Both are
//! polled against one deadline here, and the kill goes to the whole process
//! group so the pipe's real holder is what dies.
//!
//! `dot_local/bin/executable_ssh-hardening.sh` is where this repo first solved
//! this class of hang, and this is that watchdog in Rust: poll in short ticks,
//! TERM the group, wait a grace, then KILL.

use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use super::drain::Drain;

/// How often the watchdog looks at the child and its two pipes. Finer than
/// ssh-hardening.sh's 0.25s, which is the resolution of an operator-facing
/// countdown; this one is only the granularity a deadline fires at, and the
/// wake costs nothing against a lane measured in hours.
const WATCHDOG_TICK: Duration = Duration::from_millis(25);

/// How long a process group is given to die on TERM before it gets KILL,
/// the same two seconds ssh-hardening.sh waits.
pub(super) const TERM_GRACE: Duration = Duration::from_secs(2);

/// How a bounded spawn ended, which is not the same question as how the child
/// exited: a deadline that fired and a deadline that fired and did NOT stop
/// what it aimed at are different facts, and only one of them is safe to
/// report as "killed".
#[derive(Debug, PartialEq, Eq)]
pub enum Ended {
    /// The child finished on its own, inside the budget.
    Exited(ExitStatus),
    /// The budget ran out and the group is gone: both pipes reached EOF, so
    /// the collected output is everything the child produced.
    Stopped,
    /// The budget ran out and something in the group outlived TERM and KILL (a
    /// descendant that changed session, a process in uninterruptible sleep).
    /// It may still be running and still writing, so the collected output is
    /// whatever had arrived by the time this gave up.
    Escaped,
    Interrupted,
    InterruptedEscaped,
}

/// Wait for the child AND both of its pipes, or stop the whole group once
/// `budget` runs out.
pub(super) fn wait_bounded(
    child: &mut Child,
    output: &Drain,
    errors: &Drain,
    budget: Duration,
    grace: Duration,
) -> Ended {
    if settle(child, output, errors, budget, true)
        && crate::interruption().is_none()
        && let Ok(Some(status)) = child.try_wait()
    {
        return Ended::Exited(status);
    }
    let interrupted = crate::interruption().is_some();
    signal_group(child.id(), libc::SIGTERM);
    let mut settled = settle(child, output, errors, grace, false);
    // Retain the leader until the final signal. Even with both pipes closed,
    // descendants can still be alive in this group and must not outlive the lock.
    signal_group(child.id(), libc::SIGKILL);
    if !settled {
        settled = settle(child, output, errors, grace, false);
    }
    let _ = child.try_wait();
    let gone = group_gone(child.id(), grace);
    match (interrupted, settled && gone) {
        (true, true) => Ended::Interrupted,
        (true, false) => Ended::InterruptedEscaped,
        (false, true) => Ended::Stopped,
        (false, false) => Ended::Escaped,
    }
}

fn settle(
    child: &Child,
    output: &Drain,
    errors: &Drain,
    grace: Duration,
    cancellable: bool,
) -> bool {
    let started = Instant::now();
    loop {
        if cancellable && crate::interruption().is_some() {
            return false;
        }
        if output.at_eof() && errors.at_eof() && exited_without_reaping(child.id()) {
            return true;
        }
        if started.elapsed() >= grace {
            return false;
        }
        std::thread::sleep(WATCHDOG_TICK);
    }
}

fn exited_without_reaping(pid: u32) -> bool {
    // SAFETY: zeroed siginfo is valid for waitid to fill. WNOWAIT reserves our
    // unreaped child's id until all group signals finish; WNOHANG never blocks.
    unsafe {
        let mut info: libc::siginfo_t = std::mem::zeroed();
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
        ) == 0
            && info.si_pid() != 0
    }
}

fn group_gone(pid: u32, grace: Duration) -> bool {
    let started = Instant::now();
    loop {
        // SAFETY: signal zero only observes the group; it delivers no signal.
        if unsafe { libc::kill(-(pid as i32), 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            return true;
        }
        if started.elapsed() >= grace {
            return false;
        }
        std::thread::sleep(WATCHDOG_TICK);
    }
}

/// Signal the child's whole PROCESS GROUP, which is what makes the deadline
/// bound the hang rather than only the child: the pipe is held open by what
/// the child left behind, and a kill aimed at the child alone leaves that
/// running and the read blocked.
///
/// The caller must not have reaped the leader yet; `settle` says why.
fn signal_group(child: u32, signal: i32) {
    let Ok(group) = i32::try_from(child) else {
        return;
    };
    // SAFETY: `kill` against a process group this process created and still
    // holds un-reaped, with a signal number out of libc's own constants.
    unsafe { libc::kill(-group, signal) };
}
