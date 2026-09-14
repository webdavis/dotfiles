use super::*;
use std::cell::Cell;
thread_local! {static CANCEL_AT: Cell<Option<Instant>> = const {Cell::new(None)};}
fn cancelled() -> bool {
    CANCEL_AT.get().is_some_and(|at| Instant::now() >= at)
}

#[test]
fn pending_cancellation_never_starts_another_command() {
    CANCEL_AT.set(Some(Instant::now()));
    let result = SystemRunner::new(Duration::from_millis(80))
        .with_cancellation(cancelled)
        .run(
            Path::new("/bin/sh"),
            &["-c".as_ref(), "printf should-not-run".as_ref()],
            CommandIo::Inspection { merge_stderr: true },
        );
    CANCEL_AT.set(None);
    assert_eq!(result, Err(InspectionFailure::Failed));
}
#[test]
fn cancellation_stops_the_owned_group_before_the_longer_command_deadline() {
    CANCEL_AT.set(Some(Instant::now() + Duration::from_millis(60)));
    let start = Instant::now();
    let result = SystemRunner::new(Duration::from_millis(300))
        .with_termination_grace(Duration::from_millis(10))
        .with_cancellation(cancelled)
        .run(
            Path::new("/bin/sh"),
            &["-c".as_ref(), "trap '' TERM; while :; do :; done".as_ref()],
            CommandIo::Inspection { merge_stderr: true },
        );
    CANCEL_AT.set(None);
    assert_eq!(result, Err(InspectionFailure::Failed));
    assert!(start.elapsed() < Duration::from_millis(200));
}
