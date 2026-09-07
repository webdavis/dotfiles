//! The watchdog that bounds one lane subject: the spawn, the deadline, and
//! the group kill that enforces it.
//!
//! THREE FILES, three questions. This one SPAWNS, under a bound the caller can
//! give up on; `wait` decides how long the child and its pipes may take and
//! stops the group when they take longer; `drain` collects what the pipes said
//! without ever blocking the watchdog on a read.

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod drain;
mod wait;

use drain::Drain;
use wait::{TERM_GRACE, wait_bounded};

pub use wait::Ended;

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
