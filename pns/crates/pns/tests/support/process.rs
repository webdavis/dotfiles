use std::process::{Command, Output};

/// Run to completion, asserting the exit-0 edge: a failed notification must
/// never fail the caller. Every call site here is a hook path, which always
/// answers 0; a caller that expects a different code uses [`run_expecting`].
pub fn run(command: &mut Command) -> Output {
    run_expecting(0, command)
}

/// Run to completion, asserting the exit code named here. For the few
/// callers on the delivery path whose page never lands, so `run`'s default
/// stays pinned to the success edge everywhere else.
pub fn run_expecting(code: i32, command: &mut Command) -> Output {
    let output = command.output().expect("the engine runs");
    assert_eq!(output.status.code(), Some(code), "{output:?}");
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
