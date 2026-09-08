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

/// THE ANIMATION-UPKEEP PRIMITIVE, proven as a behavior rather than as a
/// function: it repeats, and then the lease stops it without anybody saying so.
#[test]
fn a_repeating_job_keeps_firing_until_its_lease_runs_out_then_stops() {
    let sandbox = Sandbox::new("daemon-repeats-until-the-lease");
    // STRUCTURAL, at ~5.4 s: the `--until +3` lease below (the comment there
    // explains why +1 flaked), plus `every` at MIN_EVERY_SECS (1 s,
    // daemon.rs: an epoch-second lease cannot lapse faster), plus a 1.2 s
    // settle that has to outlast one `every` to prove firing really stopped.
    // Three numbers add to the floor; the settle alone understates it by
    // four seconds.
    sandbox.allow_slow(
        "a 3s lease, a 1s minimum `every`, and a 1.2s settle past one `every`: ~5.4s, not just the settle",
    );
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let scheduled = schedule(
        &sandbox,
        // `--until +3` RATHER THAN `+1`. At `+1` the whole margin was "the
        // daemon starts inside the same whole second the schedule read": a
        // first drain landing in the next second fires once, re-arms past its
        // own lease, and `fires >= 2` can then never be satisfied, so the test
        // burns its ten-second deadline and fails. Measured 1 in 10 red with
        // 200ms of injected start-up delay and 5 in 10 at 300ms, on a machine
        // faster than the shared CI runner. Three seconds still proves both
        // halves: it repeats, and the lease is what stops it.
        &[
            "--id", "upkeep", "--in", "0", "--every", "1", "--until", "+3",
        ],
        &EVENT,
    );
    assert!(scheduled.status.success(), "{scheduled:?}");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| (fires(&sandbox) >= 2).then_some(())).is_some(),
        "a repeat fired {} times; the daemon said: {}",
        fires(&sandbox),
        guard.said()
    );
    // The lease runs out and the daemon drops the job of its own accord.
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the lease never expired the job: {:?}",
        spooled(&sandbox)
    );
    // THE LAST OCCURRENCE IS STILL BEING DELIVERED when the spool empties: the
    // daemon re-arms and spawns, and it never waits for the child, so the
    // firing that emptied the spool has not been recorded yet. The daemon says
    // NOTHING about a firing that worked, so the count settles itself: a window
    // longer than one `every` with no change across it is a delivery that has
    // landed and a lease that is really gone. The window is what a repeat
    // outliving its own lease would show up in.
    let settled = poll_until(|| {
        let before = fires(&sandbox);
        std::thread::sleep(Duration::from_millis(1_200));
        (fires(&sandbox) == before).then_some(before)
    })
    .expect("the firing count never stopped rising");
    assert!(
        settled >= 2,
        "the repeat fired {settled} times before the lease stopped it"
    );
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
