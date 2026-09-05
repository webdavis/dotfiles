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
    if let Some(status) = settle(child, output, errors, budget) {
        return Ended::Exited(status);
    }
    // TERM, A GRACE, THEN KILL, the shape ssh-hardening.sh uses: a subject
    // given the chance to unwind releases what it holds, and one that ignores
    // the signal still goes.
    signal_group(child.id(), libc::SIGTERM);
    let mut settled = settle(child, output, errors, grace).is_some();
    if !settled {
        signal_group(child.id(), libc::SIGKILL);
        settled = settle(child, output, errors, grace).is_some();
    }
    // The reap comes AFTER both kills and never before (see `settle`). By here
    // no further signal is aimed at this group, so releasing its id is safe.
    let _ = child.try_wait();
    if settled {
        Ended::Stopped
    } else {
        Ended::Escaped
    }
}

/// Poll until the child has exited and both pipes have reached EOF, or `grace`
/// runs out first.
fn settle(
    child: &mut Child,
    output: &Drain,
    errors: &Drain,
    grace: Duration,
) -> Option<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = exited_with_its_pipes_closed(child, output, errors) {
            return Some(status);
        }
        if started.elapsed() >= grace {
            return None;
        }
        std::thread::sleep(WATCHDOG_TICK);
    }
}

/// The child's status, ASKED FOR ONLY once both pipes are at EOF.
///
/// THE PIPE CHECKS ARE A PRECONDITION OF ASKING, never a filter on the answer,
/// because `try_wait` REAPS. ssh-hardening.sh's watchdog leaves its child
/// un-reaped until after both kills for this reason and says why: a process
/// group id stays reserved only while the group still has a member, and a
/// reaped leader whose descendants have all called `setsid` leaves the group
/// empty and its id free to be handed to somebody else. Signalling that
/// negative id would then reach an unrelated group. In the hang this watchdog
/// exists for the pipes never reach EOF, so the reap below is never reached,
/// so the id cannot be recycled underneath the kills. A child that outlives
/// its own closed pipes keeps the group alive on its own account.
fn exited_with_its_pipes_closed(
    child: &mut Child,
    output: &Drain,
    errors: &Drain,
) -> Option<ExitStatus> {
    if !output.at_eof() || !errors.at_eof() {
        return None;
    }
    child.try_wait().ok().flatten()
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
