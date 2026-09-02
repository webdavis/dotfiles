//! The watchdog that bounds one child: its deadline, and the group kill that
//! enforces it.
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

use std::io::Read;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the watchdog looks at the child and its two pipes. Finer than
/// ssh-hardening.sh's 0.25s, which is the resolution of an operator-facing
/// countdown; this one is only the granularity a deadline fires at, and the
/// wake costs nothing against a lane measured in hours.
const WATCHDOG_TICK: Duration = Duration::from_millis(25);

/// How long a process group is given to die on TERM before it gets KILL,
/// the same two seconds ssh-hardening.sh waits.
const TERM_GRACE: Duration = Duration::from_secs(2);

/// Wait for the child AND both of its pipes, or `None` once `budget` runs out,
/// in which case the child's whole process group has been killed.
fn wait_bounded(
    child: &mut Child,
    output: &Drain,
    errors: &Drain,
    budget: Duration,
) -> Option<ExitStatus> {
    if let Some(status) = settle(child, output, errors, budget) {
        return Some(status);
    }
    // TERM, A GRACE, THEN KILL, the shape ssh-hardening.sh uses: a subject
    // given the chance to unwind releases what it holds, and one that ignores
    // the signal still goes.
    signal_group(child.id(), libc::SIGTERM);
    if settle(child, output, errors, TERM_GRACE).is_none() {
        signal_group(child.id(), libc::SIGKILL);
        settle(child, output, errors, TERM_GRACE);
    }
    None
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
        if let Ok(Some(status)) = child.try_wait()
            && output.at_eof()
            && errors.at_eof()
        {
            return Some(status);
        }
        if started.elapsed() >= grace {
            return None;
        }
        std::thread::sleep(WATCHDOG_TICK);
    }
}

/// Signal the child's whole PROCESS GROUP, which is what makes the deadline
/// bound the hang rather than only the child: the pipe is held open by what
/// the child left behind, and a kill aimed at the child alone leaves that
/// running and the read blocked.
///
/// SAFE EVEN THOUGH THE CHILD MAY ALREADY BE REAPED. A pid is never handed to
/// a new process while it is still in use as a process group id, so the
/// negative pid either reaches the group members still running or fails with
/// ESRCH. It cannot reach an unrelated process.
fn signal_group(child: u32, signal: i32) {
    let Ok(group) = i32::try_from(child) else {
        return;
    };
    // SAFETY: `kill` against a process group this process created, with a
    // signal number out of libc's own constants.
    unsafe { libc::kill(-group, signal) };
}

/// What one bounded spawn produced. `status` is `None` when the DEADLINE is
/// what ended it: the child's whole process group was killed, and the two
/// buffers hold whatever it managed to print before that.
pub struct Finished {
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Drive `child` to its end inside `budget`, draining both of its pipes.
///
/// THE CHILD MUST ALREADY BE IN A PROCESS GROUP OF ITS OWN (the caller spawns
/// it with `process_group(0)`), because the kill below is aimed at that group
/// and a shared one would take the caller with it.
pub fn bounded_output(child: &mut Child, budget: Duration) -> Finished {
    let output = Drain::new(child.stdout.take());
    let errors = Drain::new(child.stderr.take());
    let status = wait_bounded(child, &output, &errors, budget);
    Finished {
        status,
        stdout: output.taken(),
        stderr: errors.taken(),
    }
}

/// One of the child's pipes, drained on a thread of its own into a buffer the
/// watchdog can take at any moment.
///
/// NEVER JOINED. The read is exactly what blocks when something the child left
/// behind still holds the pipe, so a watchdog that joined to collect the
/// output would inherit the hang it exists to bound. The group kill is what
/// closes the last write end, and the thread then ends on its own.
struct Drain {
    collected: Arc<Mutex<Vec<u8>>>,
    reader: JoinHandle<()>,
}

impl Drain {
    fn new<R: Read + Send + 'static>(pipe: Option<R>) -> Self {
        let collected = Arc::new(Mutex::new(Vec::new()));
        let into = Arc::clone(&collected);
        let reader = std::thread::spawn(move || {
            let Some(mut pipe) = pipe else { return };
            let mut chunk = [0u8; 8192];
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                lock(&into).extend_from_slice(&chunk[..read]);
            }
        });
        Drain { collected, reader }
    }

    fn at_eof(&self) -> bool {
        self.reader.is_finished()
    }

    fn taken(&self) -> Vec<u8> {
        lock(&self.collected).clone()
    }
}

/// The buffer, whatever a panicking reader left in it: a poisoned lock still
/// holds the output this child produced, and dropping it would cost the record
/// the only line that says how far the lane got.
fn lock(buffer: &Mutex<Vec<u8>>) -> MutexGuard<'_, Vec<u8>> {
    buffer.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    /// Run `call` on a thread of its own and give up on it after `grace`.
    ///
    /// EVERY DEADLINE TEST NEEDS THIS. The bug each one hunts turns the call
    /// into a HANG, and a hung test is a suite that never reports at all
    /// rather than one that goes red.
    pub(crate) fn within<T: Send + 'static>(
        grace: Duration,
        call: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || send.send(call()));
        receive
            .recv_timeout(grace)
            .expect("the call never returned: its deadline did not fire")
    }

    /// Whether a pid is still around. Used on a process that is NOT this
    /// process's child, which init reaps, so its disappearance is observable.
    fn alive(pid: i32) -> bool {
        // SAFETY: signal 0 performs no delivery; it only asks whether the pid
        // could be signalled.
        unsafe { libc::kill(pid, 0) == 0 }
    }

    /// Poll for `pid` to go away, up to `grace`.
    fn gone_within(pid: i32, grace: Duration) -> bool {
        let started = Instant::now();
        while alive(pid) {
            if started.elapsed() >= grace {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    /// A shell script in a process group of its own, spawned the way
    /// `SystemRunner` spawns a lane subject.
    fn grouped(script: &str) -> std::process::Child {
        Command::new("/bin/sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("the shell")
    }

    /// A budget spent almost at once, so a deadline test is over in a fraction
    /// of a second.
    const IMPATIENT: Duration = Duration::from_millis(200);

    #[test]
    fn a_child_that_outlives_the_budget_is_stopped_at_it() {
        let mut child = grouped("sleep 30");
        let finished = bounded_output(&mut child, IMPATIENT);
        assert!(
            finished.status.is_none(),
            "a child stopped by the budget has no status of its own"
        );
    }

    #[test]
    fn a_child_that_finishes_inside_the_budget_keeps_its_own_status_and_output() {
        let mut child = grouped("printf 'said this\\n'; printf 'and this\\n' >&2; exit 3");
        let finished = bounded_output(&mut child, Duration::from_secs(30));
        assert_eq!(finished.status.and_then(|status| status.code()), Some(3));
        assert_eq!(finished.stdout, b"said this\n");
        assert_eq!(finished.stderr, b"and this\n");
    }

    #[test]
    fn a_child_that_exits_while_a_grandchild_holds_the_pipe_is_still_stopped_at_the_budget() {
        // THE HANG THIS EXISTS FOR, and it is not simply a slow child: the
        // child exits at once and something it left behind keeps stdout open,
        // so waiting on the child returns immediately and the READ is what
        // blocks. THE GRANDCHILD OUTLIVES THE WHOLE WATCHDOG on purpose: at 30
        // seconds it cannot exit on its own inside the budget plus both kill
        // graces, so a call that returns here returned because uu stopped it.
        let finished = within(Duration::from_secs(3), || {
            let mut child = grouped("sleep 30 & printf 'got this far\\n'; exit 0");
            bounded_output(&mut child, IMPATIENT)
        });
        assert!(finished.status.is_none(), "the budget is what ended this");
        // WHAT IT PRINTED IS KEPT: those lines are how far the child got, and
        // they are the whole of what anyone has to diagnose a hang with.
        assert_eq!(finished.stdout, b"got this far\n");
    }

    #[test]
    fn the_budget_kills_the_whole_process_group_and_not_only_the_child() {
        // What the pipe is actually held open by is a GRANDCHILD, so a kill
        // aimed at the child alone leaves it running and the read blocked.
        let finished = within(Duration::from_secs(3), || {
            let mut child = grouped("sleep 30 & echo $!; exit 0");
            bounded_output(&mut child, IMPATIENT)
        });
        let grandchild: i32 = String::from_utf8_lossy(&finished.stdout)
            .trim()
            .parse()
            .unwrap_or_else(|_| {
                panic!(
                    "expected the grandchild's pid, got {:?}",
                    String::from_utf8_lossy(&finished.stdout)
                )
            });
        assert!(
            gone_within(grandchild, Duration::from_secs(1)),
            "the grandchild survived the deadline, so only the child was killed"
        );
    }
}
