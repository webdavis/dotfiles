use super::*;
use crate::test_sandbox::Sandbox;
use std::fs;
use std::process::Command;

/// A HANG GUARD RATHER THAN A MEASUREMENT: every wait below leaves on the
/// event it waits for, and this only stops a wedged fixture.
const HANG_GUARD: Duration = Duration::from_secs(30);

#[test]
fn the_deadline_sends_term_before_kill_and_retains_timeout_outcome() {
    let sandbox = Sandbox::new("term-marker");
    let signalled = sandbox.path().join("signalled");
    let ready = sandbox.path().join("ready");
    // THE STOP PATH IS ENTERED ONCE THE CHILD SAYS ITS TRAP IS INSTALLED. A
    // 60ms deadline used to send the signal whether or not `sh` had reached
    // its `trap` line, and a 30ms grace used to kill the handler mid-write;
    // both read on a loaded machine as a marker this runner never sent.
    let mut child = OwnedChild(Some(
        Command::new("/bin/sh")
            .args([
                OsStr::new("-c"),
                OsStr::new(
                    "trap 'printf term >\"$1\"; exit 0' TERM; printf ready >\"$2\"; while :; do :; done",
                ),
                OsStr::new("fixture"),
                signalled.as_os_str(),
                ready.as_os_str(),
            ])
            .process_group(0)
            .spawn()
            .expect("the fixture child runs"),
    ));
    let until = Instant::now() + HANG_GUARD;
    while !ready.exists() {
        assert!(
            Instant::now() < until,
            "the fixture never installed its TERM trap"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    child.stop(HANG_GUARD);
    assert_eq!(
        fs::read(&signalled).ok().as_deref(),
        Some(b"term".as_slice())
    );

    // AND THE DEADLINE ITSELF, which stops a child that never exits the same
    // way and still reports the run as timed out.
    let mut runner = SystemRunner::per_command(Duration::from_millis(60))
        .with_termination_grace(Duration::from_millis(30));
    let result = runner.run_completed(
        Path::new("/bin/sh"),
        &["-c".as_ref(), "while :; do :; done".as_ref()],
        CommandIo::Inspection { merge_stderr: true },
    );
    assert_eq!(result, Err(InspectionFailure::TimedOut));
}

#[test]
fn term_ignoring_child_and_grandchild_are_killed_and_reaped_after_the_grace() {
    let sandbox = Sandbox::new("owned-group");
    let path = sandbox.path().join("pids");
    let mut runner = SystemRunner::per_command(Duration::from_millis(70))
        .with_termination_grace(Duration::from_millis(25));
    let start = Instant::now();
    let result = runner.run_completed(
        Path::new("/bin/sh"),
        &[
            "-c".as_ref(),
            "trap '' TERM; /bin/sleep 600 & printf '%s\\n%s\\n' \"$$\" \"$!\" >\"$1\"; wait"
                .as_ref(),
            "fixture".as_ref(),
            path.as_os_str(),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    let pids: Vec<i32> = fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|pid| pid.parse().unwrap())
        .collect();
    // Reaping a killed descendant is the kernel's own work, so it is polled for rather than
    // timed: the loop leaves as soon as they are gone and the deadline only bounds a failure.
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        // Only this fixture's recorded, owned processes are queried or cleaned up.
        let live: Vec<_> = pids
            .iter()
            .filter(|pid| unsafe { libc::kill(**pid, 0) == 0 })
            .collect();
        if live.is_empty() {
            break;
        }
        if Instant::now() >= until {
            for pid in live {
                unsafe {
                    libc::kill(*pid, libc::SIGKILL);
                }
            }
            panic!("owned descendants survived timeout");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(result, Err(InspectionFailure::TimedOut));
    // A lower bound only ever grows under load: the kill cannot have preceded the grace.
    assert!(start.elapsed() >= Duration::from_millis(95));
}

#[test]
fn graceful_runner_keeps_healthy_exit_status_and_supplies_stdin_eof() {
    let mut runner = SystemRunner::per_command(Duration::from_millis(100))
        .with_termination_grace(Duration::from_secs(2));
    let output = runner.run_completed(
        Path::new("/bin/sh"),
        &[
            "-c".as_ref(),
            "if read value; then exit 1; fi; printf eof; exit 7".as_ref(),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    assert_eq!(
        output,
        Ok(CommandOutput {
            bytes: b"eof".to_vec(),
            exit: 7
        })
    );
}
