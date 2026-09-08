use super::*;

/// A FIFO IN THE SPOOL MUST NOT STALL THE CLOCK. `open` on one blocks until a
/// writer arrives, so a daemon that opened what it found would stop forever on
/// the first tick that saw it, with nothing in the log to say why.
///
/// ADDED BEYOND THE BRIEF'S FIFTEEN: the brief specifies this as one of the
/// loop's four refusals rather than as a behavior, and a refusal whose failure
/// is an unkillable hang is worth the one test.
#[test]
fn an_irregular_spool_entry_is_left_alone_and_never_opened() {
    let sandbox = Sandbox::new("daemon-refuses-a-fifo");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let spool = sandbox.state().join("daemon");
    std::fs::create_dir_all(&spool).expect("the spool");
    let fifo = spool.join("not-a-job");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|status| status.success()),
        "the test needs a real FIFO"
    );

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        schedule(&sandbox, &["--id", "ordinary", "--in", "0"], &EVENT)
            .status
            .success()
    );
    assert!(
        poll_until(|| (fires(&sandbox) > 0).then_some(())).is_some(),
        "the FIFO stalled the clock; the daemon said: {}",
        guard.said()
    );
    // Left exactly where it was found, and complained about once rather than
    // once a tick.
    assert!(fifo.exists(), "an irregular entry is never removed");
    assert_eq!(
        guard
            .said()
            .lines()
            .filter(|line| line.contains("not a regular file"))
            .count(),
        1,
        "said once, not once a tick: {}",
        guard.said()
    );
}

/// M17'S BEHAVIOR, WHICH NOTHING EXERCISED: a marker file actually on disk
/// cancels a scheduled job.
///
/// The decision function's boolean was unit tested, and `marker_exists` and
/// `marker_dir` had no reference in any test at all, so pointing the markers
/// directory at a name that does not exist survived the whole suite. This is
/// the nag's entire cancellation primitive, and the nag slice is queued
/// directly on top of this one.
#[test]
fn a_marker_on_disk_cancels_a_scheduled_job_end_to_end() {
    let sandbox = Sandbox::new("daemon-marker-cancels");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let markers = sandbox.state().join("daemon-markers");
    std::fs::create_dir_all(&markers).expect("the markers directory");
    std::fs::write(markers.join("answered"), "").expect("the marker");

    assert!(
        schedule(
            &sandbox,
            &["--id", "nag", "--in", "0", "--unless-marker", "answered"],
            &EVENT,
        )
        .status
        .success()
    );
    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the cancelled job was never taken out of the spool: {:?}",
        spooled(&sandbox)
    );
    assert!(
        guard.said().contains("its marker was already there"),
        "the drop must name the marker as the reason: {}",
        guard.said()
    );

    // THE IN-TEST CONTROL: the same daemon, the same tick, an identical job
    // with no marker naming it. It fires, so the silence above is the marker
    // and not a daemon that was never running.
    assert!(
        schedule(&sandbox, &["--id", "ordinary", "--in", "0"], &EVENT)
            .status
            .success()
    );
    assert!(
        poll_until(|| (fires(&sandbox) == 1).then_some(())).is_some(),
        "the unmarked control never fired; the daemon said: {}",
        guard.said()
    );
    std::thread::sleep(Duration::from_millis(TICK_MS * 8));
    assert_eq!(
        fires(&sandbox),
        1,
        "only the unmarked job may have fired; the daemon said: {}",
        guard.said()
    );
}

/// M18'S BEHAVIOR: a hand-edited spool record that would not have passed
/// registration is dropped rather than run.
///
/// The loop re-applies the registration's own validation to what it reads back,
/// which is what stops a file written by hand from doing what a registration
/// could not. Nothing wrote a malformed spool file, so deleting that re-check
/// survived the whole suite.
#[test]
fn a_hand_edited_spool_record_whose_args_fail_validation_is_dropped() {
    let sandbox = Sandbox::new("daemon-drops-a-hand-edited-record");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let spool = sandbox.state().join("daemon");
    std::fs::create_dir_all(&spool).expect("the spool");
    let now = now_secs();
    // Parses cleanly and is refused by the shape rules: an empty argv is a job
    // that would re-execute pns with no event at all.
    std::fs::write(
        spool.join("handmade"),
        format!(
            "id=handmade	due={now}	until={}	args=[]
",
            now + 300
        ),
    )
    .expect("the hand-edited record");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the invalid record was left in the spool: {:?}",
        spooled(&sandbox)
    );
    assert!(
        guard.said().contains("dropped `handmade`") && guard.said().contains("`args` is empty"),
        "the drop must name the record and the rule it broke: {}",
        guard.said()
    );
    assert_eq!(fires(&sandbox), 0, "a refused record must never be run");
}

/// M21'S BEHAVIOR: a spool path that is not a directory refuses the start, and
/// the refusal EXITS 0.
///
/// Neither startup refusal can heal by retrying, and under
/// `KeepAlive { SuccessfulExit = false }` a non-zero exit is relaunched every
/// ten seconds forever: ~8,640 relaunches and ~8,640 copies of this line a day,
/// which is the chatter the no-log-per-tick behavior exists to prevent arriving
/// through the restart door instead.
#[test]
fn a_spool_that_is_not_a_directory_refuses_the_start_and_exits_zero() {
    let sandbox = Sandbox::new("daemon-refuses-a-spool-file");
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    std::fs::write(state.join("daemon"), "not a directory").expect("a file in the way");

    let output = sandbox
        .pns_stateful()
        .env("PNS_DAEMON_TICK_MS", TICK_MS.to_string())
        .args(["daemon", "run"])
        .output()
        .expect("the engine runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a refusal retrying cannot fix must not be relaunched every ten seconds"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not a directory; refusing to start"),
        "the refusal must say what it found: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
