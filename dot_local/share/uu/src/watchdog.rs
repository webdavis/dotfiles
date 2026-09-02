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
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
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
fn wait_bounded(
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

/// What one bounded spawn produced.
pub struct Finished {
    pub ended: Ended,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// How long past its own budget a bounded spawn may take to answer before the
/// SPAWN itself is judged stuck. `bounded_output`'s own worst case is the
/// budget plus both kill graces, so anything past this means `Command::spawn`
/// never returned.
const SPAWN_SLACK: Duration = Duration::from_secs(8);

/// What came of trying to run one program under a budget.
pub enum Spawned {
    Ran(Finished),
    /// The program could not be started at all, already fit to print.
    NotRunnable(String),
    /// `Command::spawn` ITSELF never returned. No pid exists in that case, so
    /// nothing can be signalled and only the caller giving up bounds it.
    SpawnStuck,
}

/// Run `program` under `budget`, in a process group of its own, collecting
/// both of its pipes.
///
/// THE SPAWN IS INSIDE THE BOUND, on a thread the caller can give up on.
/// `Command::spawn` is synchronous and its exec can block on a filesystem that
/// stopped answering, and until it returns there is no pid to signal, so a
/// watchdog started afterwards would never run and uu's run lock would be held
/// for good. An abandoned thread finishes the job itself: by the time its
/// spawn returns the budget is spent, so the `bounded_output` it then enters
/// stops the group it just created.
///
/// ONLY WHILE UU LIVES, measured: `main` ends at `std::process::exit`, so a
/// spawn whose kernel-side child already exists and that returns after uu has
/// finished leaves it orphaned. Joining instead is the unbounded hang this
/// exists to end, so that orphan is the accepted price.
pub fn bounded_spawn(program: &str, args: &[&str], stdin: Stdio, budget: Duration) -> Spawned {
    let (send, receive) = std::sync::mpsc::channel();
    let owned_program = program.to_string();
    let owned_args: Vec<String> = args.iter().map(|word| (*word).to_string()).collect();
    std::thread::spawn(move || {
        let started = Instant::now();
        let spawned = Command::new(&owned_program)
            .args(&owned_args)
            .stdin(stdin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // A PROCESS GROUP OF ITS OWN, which is what lets the kill below
            // reach whatever the child leaves behind. The cost is that an
            // interactive Ctrl-C no longer reaches the subject through uu's
            // own group; under the launchd job that carries these lanes there
            // is no terminal to send one.
            .process_group(0)
            .spawn();
        // WHAT THE SPAWN ITSELF COST comes off the budget, so a spawn that
        // took half an hour does not hand the child a fresh one.
        let _ = send.send(match spawned {
            Ok(mut child) => Spawned::Ran(bounded_output(
                &mut child,
                budget.saturating_sub(started.elapsed()),
            )),
            Err(error) => Spawned::NotRunnable(format!("could not run {owned_program}: {error}")),
        });
    });
    receive
        .recv_timeout(budget + SPAWN_SLACK)
        .unwrap_or(Spawned::SpawnStuck)
}

/// Drive `child` to its end inside `budget`, draining both of its pipes.
///
/// THE CHILD MUST ALREADY BE IN A PROCESS GROUP OF ITS OWN (the caller spawns
/// it with `process_group(0)`), because the kill below is aimed at that group
/// and a shared one would take the caller with it.
fn bounded_output(child: &mut Child, budget: Duration) -> Finished {
    let output = Drain::new(child.stdout.take());
    let errors = Drain::new(child.stderr.take());
    let ended = wait_bounded(child, &output, &errors, budget, TERM_GRACE);
    Finished {
        ended,
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

    /// The kill grace under test, short for the same reason: the production
    /// two seconds is a courtesy to a subject unwinding, and nothing here
    /// unwinds.
    const BRIEF_GRACE: Duration = Duration::from_millis(200);

    /// `bounded_output` with the grace shortened, which is the only thing a
    /// test needs to move.
    fn bounded(child: &mut std::process::Child, budget: Duration) -> Finished {
        let output = Drain::new(child.stdout.take());
        let errors = Drain::new(child.stderr.take());
        let ended = wait_bounded(child, &output, &errors, budget, BRIEF_GRACE);
        Finished {
            ended,
            stdout: output.taken(),
            stderr: errors.taken(),
        }
    }

    #[test]
    fn a_spawn_that_reaches_its_watchdog_with_the_budget_already_spent_still_stops_the_group() {
        // THE ABANDONED THREAD'S OWN PATH. A spawn returning after the caller
        // gave up enters `bounded_output` with a ZERO budget, which nothing
        // else here reaches: `SystemRunner` refuses to spawn on a spent one.
        let spawned = within(Duration::from_secs(5), || {
            bounded_spawn(
                "/bin/sh",
                &["-c", "sleep 30"],
                Stdio::null(),
                Duration::ZERO,
            )
        });
        let Spawned::Ran(finished) = spawned else {
            panic!("the shell is there, so this ran");
        };
        // `Stopped` IS the proof the group is gone: it is reported only once
        // both pipes reached EOF, which a group still holding them cannot do.
        assert_eq!(finished.ended, Ended::Stopped);
    }

    #[test]
    fn a_child_that_outlives_the_budget_is_stopped_at_it() {
        let mut child = grouped("sleep 30");
        assert_eq!(bounded(&mut child, IMPATIENT).ended, Ended::Stopped);
    }

    #[test]
    fn a_child_that_finishes_inside_the_budget_keeps_its_own_status_and_output() {
        let mut child = grouped("printf 'said this\\n'; printf 'and this\\n' >&2; exit 3");
        let finished = bounded(&mut child, Duration::from_secs(30));
        let Ended::Exited(status) = finished.ended else {
            panic!(
                "this child exits on its own, it was not {:?}",
                finished.ended
            );
        };
        assert_eq!(status.code(), Some(3));
        assert_eq!(finished.stdout, b"said this\n");
        assert_eq!(finished.stderr, b"and this\n");
    }

    #[test]
    fn a_child_that_ignores_term_is_killed_rather_than_left_running() {
        // TERM ALONE IS NOT ENOUGH, and every other fixture here is a `sleep`,
        // which dies on the first signal: without a subject that IGNORES TERM
        // the KILL escalation is unreachable and could be deleted unnoticed.
        let mut child = grouped("trap '' TERM; sleep 30");
        let started = Instant::now();
        assert_eq!(bounded(&mut child, IMPATIENT).ended, Ended::Stopped);
        // The TERM had to be given its whole grace first, so a run that came
        // back inside it would mean the first signal is what stopped this.
        assert!(
            started.elapsed() >= IMPATIENT + BRIEF_GRACE,
            "this returned in {:?}, too soon to have waited out the TERM grace",
            started.elapsed()
        );
    }

    #[test]
    fn a_pipe_holder_outside_the_group_is_reported_as_escaped_not_as_killed() {
        // The kill reaches a GROUP, so anything that left the group (a
        // descendant that called setsid) outlives it and keeps writing after
        // uu drops the run lock. Reporting that as "its process group was
        // killed" would be a clean stop uu never verified, so the holder here
        // is put in a group of its own and the pipe never reaches EOF.
        let (reader, writer) = std::io::pipe().expect("a pipe");
        let held = writer
            .try_clone()
            .expect("a second handle on the write end");
        let mut holder = Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::from(held))
            .process_group(0)
            .spawn()
            .expect("the holder");
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer))
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("the subject");
        let output = Drain::new(Some(reader));
        let errors = Drain::new(None::<std::process::ChildStderr>);
        let ended = within(Duration::from_secs(3), move || {
            wait_bounded(&mut child, &output, &errors, IMPATIENT, BRIEF_GRACE)
        });
        let _ = holder.kill();
        let _ = holder.wait();
        assert_eq!(ended, Ended::Escaped);
    }

    #[test]
    fn a_grandchild_holding_only_stderr_still_hits_the_budget() {
        // EACH PIPE IS ITS OWN CONDITION. Every other hanging fixture holds
        // stdout, so the stderr half of the wait could be dropped and the
        // suite would stay green. Here `sleep` sends its own stdout to the
        // stderr pipe, so stdout reaches EOF when the shell exits and only
        // stderr is still held.
        let finished = within(Duration::from_secs(3), || {
            let mut child = grouped("sleep 30 >&2 & printf 'on stdout\\n'; exit 0");
            bounded(&mut child, IMPATIENT)
        });
        assert_eq!(finished.ended, Ended::Stopped);
        assert_eq!(finished.stdout, b"on stdout\n");
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
            bounded(&mut child, IMPATIENT)
        });
        assert_eq!(finished.ended, Ended::Stopped);
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
            bounded(&mut child, IMPATIENT)
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
