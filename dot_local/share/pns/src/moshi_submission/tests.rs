use super::answer_within_on;
use std::cell::RefCell;
use std::time::Duration;

/// A submission that never answers, driven to its give-up on a clock that
/// moves only when the wait sleeps on it: the code that came back, the
/// clock at the return, and every sleep with the reading it began at.
///
/// THE CHILD IS REAL AND THE CLOCK IS NOT. `try_wait` needs a process, and
/// one that sleeps for a minute stays alive across the microseconds this
/// takes, so the give-up is decided by the fake clock alone. The kill is not
/// asserted here; the hooks suite pins it by the stub's stream closing.
fn silent_submission_given_up_on(deadline: Duration) -> (i32, Duration, Vec<(Duration, Duration)>) {
    let child = std::process::Command::new("/bin/sleep")
        .arg("60")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("a child that never answers");
    let clock = RefCell::new(Duration::ZERO);
    let sleeps = RefCell::new(Vec::new());
    let code = answer_within_on(
        child,
        deadline,
        || *clock.borrow(),
        |asked| {
            sleeps.borrow_mut().push((*clock.borrow(), asked));
            *clock.borrow_mut() += asked;
        },
    );
    (code, clock.into_inner(), sleeps.into_inner())
}

#[test]
fn a_silent_submission_is_given_up_on_when_the_clock_reaches_the_deadline() {
    let deadline = Duration::from_millis(150);
    let (code, clock_at_return, _) = silent_submission_given_up_on(deadline);
    assert_eq!(code, 0, "an expiry is no opinion");
    // THE BOUND HONOURED IS THE ONE HANDED IN, exactly: the deadline is a
    // whole number of poll ticks, so the clock reads it at the give-up. A
    // wait that measured against a bound of its own reads that bound here
    // whatever its sentence says.
    assert_eq!(
        clock_at_return, deadline,
        "the wait gave up at {clock_at_return:?} on a {deadline:?} deadline"
    );
}

#[test]
fn giving_up_on_a_submission_releases_the_prompt_without_waiting_further() {
    let deadline = Duration::from_millis(150);
    let (_, _, sleeps) = silent_submission_given_up_on(deadline);
    // NOT ONE SLEEP AFTER THE DEADLINE. The prompt is released the moment
    // the wait returns, so anything it waits on after giving up is time
    // the operator's prompt stays hidden for no reason.
    let after_the_deadline: Vec<_> = sleeps
        .iter()
        .filter(|(began, _)| *began >= deadline)
        .collect();
    assert!(
        after_the_deadline.is_empty(),
        "the wait kept sleeping after it gave up: {after_the_deadline:?}"
    );
}
