use super::*;

/// The slack over the deadline and the grace. The bound this pins is that the
/// wait ends at all, on a machine compiling other lanes at the same time.
const SLACK: Duration = Duration::from_secs(10);

fn sleeper() -> Command {
    let mut command = Command::new("/bin/sh");
    // An ignored TERM disposition survives the exec, so only SIGKILL ends it.
    command
        .args(["-c", "trap '' TERM; exec /bin/sleep 30"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[test]
fn a_child_that_finishes_in_time_reports_its_exit_code() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exit 7"]);
    let status = run_bounded(&mut command, Duration::from_secs(30)).unwrap();
    assert_eq!(status.code(), Some(7));
}
#[test]
fn a_child_that_outlives_the_deadline_is_killed_and_reported_as_timed_out() {
    let duration = Duration::from_millis(80);
    let started = Instant::now();
    let status = run_bounded(&mut sleeper(), duration).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(status.code(), Some(137));
    assert!(elapsed >= duration, "{elapsed:?}");
    assert!(elapsed < duration + GRACE + SLACK, "{elapsed:?}");
}
#[test]
fn a_program_that_cannot_be_spawned_is_reported() {
    let mut command = Command::new("/owned/absent/pns");
    assert!(run_bounded(&mut command, Duration::from_secs(30)).is_err());
}
