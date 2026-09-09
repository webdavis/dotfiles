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
        stdout_error: None,
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
