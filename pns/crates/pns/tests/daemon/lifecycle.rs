use super::*;

/// WHERE A NAIVE `wait()` WOULD PASS EVERY OTHER TEST AND HANG IN PRODUCTION.
#[test]
fn a_hung_child_does_not_stall_the_tick_and_is_killed() {
    let sandbox = Sandbox::new("daemon-hung-child");
    // ONE STUB, TWO BEHAVIORS, told apart by the event it was handed: the
    // hanging job records the pns process that ran it and then hangs far past
    // the daemon's bound, and any other job records that it arrived.
    sandbox.write_config(ONE_CHANNEL);
    // The hanging job also starts a GRANDCHILD of the daemon's own child and
    // records its pid, because that is what a real delivery is: the job is a
    // `pns` that spawns a channel and waits on it. Killing only the direct
    // child leaves the delivery running.
    sandbox.stub_channel(
        "hermes",
        &format!(
            "event=$(cat)\n\
             if [[ $event == *hangs* ]]; then\n\
             sleep 30 &\n\
             if mkdir \"{sandbox}/first-attempt\" 2>/dev/null; then\n\
             printf '%s' \"$!\" >\"{sandbox}/hung.grandchild\"\n\
             printf '%s' \"$PPID\" >\"{sandbox}/hung.ppid\"\n\
             else\n\
             touch \"{sandbox}/retry-started\"\n\
             fi\n\
             wait\n\
             else\n\
             printf '%s' \"$event\" >\"{sandbox}/second.event\"\n\
             fi",
            sandbox = sandbox.display()
        ),
    );

    assert!(
        schedule(
            &sandbox,
            &["--id", "hangs", "--in", "0"],
            &["--agent", "pns", "--state", "done", "--detail", "hangs"],
        )
        .status
        .success()
    );
    // FAST_TICK_MS, not TICK_MS: the kill bound below is CHILD_TICKS (30)
    // ticks deep, so the ordinary tick would cost 750 ms proving the same
    // thing this floor proves in 300.
    let guard = DaemonGuard::start(&sandbox, FAST_TICK_MS);
    let hung = poll_until(|| std::fs::read_to_string(sandbox.path("hung.ppid")).ok())
        .expect("the hung job never started");

    assert!(
        poll_until(|| (!process_lives(&hung)).then_some(())).is_some(),
        "the pns process the hung job started ({hung}) outlived the daemon's bound"
    );
    assert!(
        poll_until(|| sandbox.path("retry-started").exists().then_some(())).is_some(),
        "the retained delivery was never retried"
    );

    // The actual retry is hanging before the ordinary job is registered. A
    // synchronous retry otherwise blocks the daemon for the channel's five seconds.
    assert!(
        schedule(&sandbox, &["--id", "ordinary", "--in", "0"], &EVENT)
            .status
            .success()
    );
    // WAITING FOR THE FILE TO APPEAR, so a longer bound only gives it more
    // time and never weakens the assertion below. 200ms was tight enough to
    // fail on a loaded CI runner.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !sandbox.path("second.event").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        sandbox.path("second.event").exists(),
        "a hanging retry stalled the next job; the daemon said: {}",
        guard.said()
    );

    // AND THEN THE DELIVERY IT WAS WAITING ON, which is the half a kill aimed
    // at the direct child alone leaves running: measured still alive 750ms past
    // a 300ms bound, and a repeating hung job accumulates one every occurrence.
    let grandchild = std::fs::read_to_string(sandbox.path("hung.grandchild"))
        .expect("the hung job never recorded its own child");
    assert!(
        poll_until(|| (!process_lives(&grandchild)).then_some(())).is_some(),
        "the delivery the hung job started ({grandchild}) outlived the daemon's bound"
    );
}

/// Whether a pid is still around, asked without a signal of our own: `kill -0`
/// sends nothing and only reports existence.
fn process_lives(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", pid])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// M20'S BEHAVIOR: `enabled = false` stops a daemon that is ALREADY RUNNING.
///
/// Read once at startup the switch was inert. Nothing bounces this launchd job
/// when the config changes (the loader's trigger is the plist hash), so the
/// operator's off switch did nothing at all until a hand-typed bootout, while
/// the daemon kept firing jobs and the doctor reported it off.
///
/// EXIT 0 IS HALF THE BEHAVIOR: `KeepAlive { SuccessfulExit = false }` is what
/// keeps a clean exit exited, and a non-zero one here would relaunch the job
/// every ten seconds forever.
#[test]
fn turning_the_config_switch_off_stops_a_running_daemon() {
    let sandbox = Sandbox::new("daemon-off-switch-is-real");
    sandbox.write_config(&format!(
        "{ONE_CHANNEL}[daemon]
enabled = true
"
    ));
    // FAST_TICK_MS, not TICK_MS: the switch is re-read every SWITCH_TICKS
    // (30) ticks, so the ordinary tick would cost 750 ms proving the same
    // thing this floor proves in 300.
    let mut guard = DaemonGuard::start(&sandbox, FAST_TICK_MS);
    // Up and beating before the switch moves, so the exit below is the config
    // and not a daemon that never started.
    assert!(
        poll_until(|| sandbox
            .state()
            .join("daemon-heartbeat")
            .exists()
            .then_some(()))
        .is_some(),
        "the daemon never beat; it said: {}",
        guard.said()
    );

    sandbox.write_config(&format!(
        "{ONE_CHANNEL}[daemon]
enabled = false
"
    ));
    let status = guard
        .exited_within(Duration::from_secs(10))
        .expect("the daemon kept running after the switch was turned off");
    assert_eq!(
        status.code(),
        Some(0),
        "a switched-off daemon must exit cleanly so launchd keeps it down"
    );
    assert!(
        guard.said().contains("disabled in the config; exiting"),
        "the exit must say why: {}",
        guard.said()
    );
}
