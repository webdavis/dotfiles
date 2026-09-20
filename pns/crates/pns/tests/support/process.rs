use std::process::{Command, Output};

/// Run to completion, asserting only that the engine ANSWERED: 0 when every
/// destination took the page and 1 when one did not, which is the exit code
/// every caller hears now. A refusal (2), a crash or a signal is still a
/// failure here, and a test that cares which of 0 and 1 it got says so itself.
pub fn run(command: &mut Command) -> Output {
    let output = command.output().expect("the engine runs");
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "the engine must answer 0 or 1 on a delivery path: {output:?}"
    );
    output
}

/// Poll for something a DETACHED child produces, to a deadline, and answer
/// None when it never arrived.
///
/// A FIXED SLEEP IS REFUSED. The recap runs in a process the engine spawns and
/// never waits for, so the parent exits before the child has posted; a sleep
/// long enough to be safe makes the suite slow and a short one makes it flaky,
/// and neither says what it saw. This ends on the evidence, and the CALLER
/// reports the failure, because only the caller knows how to describe what was
/// there instead.
pub fn poll_until<T>(mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(found) = probe() {
            return Some(found);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
