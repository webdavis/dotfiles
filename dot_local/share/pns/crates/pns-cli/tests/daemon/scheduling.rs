use super::*;

/// THE TEST THAT PROVES THE FEATURE EXISTS END TO END: something was scheduled,
/// nothing else happened, and a card came out of it.
#[test]
fn a_scheduled_job_runs_once_and_its_effect_is_observable() {
    let sandbox = Sandbox::new("daemon-runs-a-job");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let scheduled = schedule(&sandbox, &["--id", "drill", "--in", "0"], &EVENT);
    assert!(scheduled.status.success(), "{scheduled:?}");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| (fires(&sandbox) > 0).then_some(())).is_some(),
        "the job never fired; the daemon said: {}",
        guard.said()
    );
    // The spool is drained by the run, and a one-shot leaves nothing behind.
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the spool still holds {:?}",
        spooled(&sandbox)
    );
    // ONCE, and it stays once: a one-shot that re-armed would keep firing.
    std::thread::sleep(Duration::from_millis(TICK_MS * 8));
    assert_eq!(fires(&sandbox), 1, "a one-shot fires exactly once");
}

/// THE POINT IS THE ABSENCE OF A WAIT. A registration that talked to the daemon
/// would hold its caller for as long as the daemon was wedged, which is the
/// class the whole design exists to stay out of.
#[test]
fn a_registration_succeeds_with_no_daemon_anywhere_and_blocks_on_nothing() {
    let sandbox = Sandbox::new("daemon-registration-never-waits");
    let started = Instant::now();
    let scheduled = schedule(&sandbox, &["--id", "lonely", "--in", "600"], &EVENT);
    let elapsed = started.elapsed();
    assert!(scheduled.status.success(), "{scheduled:?}");
    assert_eq!(spooled(&sandbox), vec!["lonely".to_string()]);
    // A GENEROUS CEILING, never a tight number: this asserts that nothing
    // waits, not how fast a process starts on a loaded machine.
    assert!(
        elapsed < Duration::from_secs(5),
        "the registration took {elapsed:?}, which is a wait on something"
    );
}
