use super::*;

#[test]
fn an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer() {
    let ordinary = registering_event("lights-tick-lease-ordinary");
    run(logged_event(&ordinary).args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    let short = lights_job(&ordinary);
    assert_eq!(
        short.args,
        vec!["lights".to_string(), "tick".to_string()],
        "the daemon re-executes THIS binary with the tick's own words"
    );
    assert_eq!(
        short.every,
        Some(20),
        "it repeats at the configured refresh"
    );

    // A JOURNALLED EVENT IS AN OPERATOR WHO IS NOT HERE, and the glow has to
    // survive the whole absence, which is precisely when no further event
    // arrives to refresh the lease.
    let away = registering_event("lights-tick-lease-journalled");
    mute(&away);
    run(logged_event(&away).args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert_eq!(journal(&away).len(), 1, "the event really was journalled");
    let long = lights_job(&away);

    // EXACT, AND NOT MERELY DIFFERENT. `until` is `due.max(now + lease)`, so a
    // `refresh_secs` longer than the ordinary lease used to EXTEND that lease to
    // the refresh: an allowed 600 seconds bought a ten-minute backstop and an
    // allowed day bought a sticky glow with no repeat left to clear it. The
    // config ceiling is what closes that, and this is the assertion that reads
    // the two lengths back. `due` is `now + refresh_secs` on a sandbox holding
    // no pending job, which is what recovers the second the lease was measured
    // from without a second clock on this side.
    const REFRESH: u64 = 20;
    assert_eq!(
        short.until - (short.due - REFRESH),
        300,
        "the ordinary lease is five minutes, whatever the refresh interval is"
    );
    assert_eq!(
        long.until - (long.due - REFRESH),
        12 * 60 * 60,
        "and a journalled event, which is an operator who is not here to send \
         another, leases twelve hours"
    );
}

#[test]
fn a_registration_that_cannot_be_written_costs_the_event_nothing() {
    // A GUARD, and it is the fail-open claim of the whole change: a lamp that
    // did not re-arm must never cost a card, a line of stdout or an exit code.
    let outcome = |name: &str, break_the_spool: bool| {
        let sandbox = registering_event(name);
        std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
        if break_the_spool {
            // A REGULAR FILE WHERE THE SPOOL DIRECTORY GOES, so every write
            // into it fails and no repair can succeed.
            std::fs::write(sandbox.path("state/daemon"), "not a directory").expect("the blockage");
        }
        let output = logged_event(&sandbox)
            .args(["--agent", "claude", "--state", "done", "--detail", "x"])
            .output()
            .expect("the engine runs");
        (
            stdout(&output).replace(&sandbox.display(), "<sandbox>"),
            stderr(&output).replace(&sandbox.display(), "<sandbox>"),
            output.status.code(),
            ["mobile", "hermes", "macos-banner"].map(|leg| sandbox.fired(leg)),
        )
    };
    let working = outcome("lights-tick-spool-fine", false);
    assert_eq!(
        (working.2, working.3),
        (Some(0), [true, true, false]),
        "the comparison only means something against a live baseline: {working:?}"
    );
    assert_eq!(
        outcome("lights-tick-spool-broken", true),
        working,
        "same stdout, same stderr, same exit code, same legs"
    );
}

#[test]
fn a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold() {
    // THE LOOP LAMP COULD NOT BE REACHED AT ALL. The tick's lease was
    // refreshed by EVENTS and by nothing else, and a plain shell command
    // produces no events: with the automatic threshold at five minutes by
    // default and six on the operator's own machine, both PAST the five-minute
    // lease an event leaves behind, the daemon dropped the job before the run
    // it was watching could ever qualify. The one lamp whose job is a long run
    // could not arm itself.
    let sandbox = Sandbox::new("lights-tick-renews-its-own-lease");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"{DEAD_BRIDGE}\"\nkey = \"k\"\n{STUDIO_MAP}"
    ));
    // A COMMAND THIS TEST'S OWN PROCESS IS HOLDING: the sweep reads the pid in
    // the name and only a LIVE shell's marker counts as work in flight.
    let shell = sandbox.path("state/lights-shell");
    std::fs::create_dir_all(&shell).expect("the shell marker directory");
    let started = now_secs() - 100;
    std::fs::write(
        shell.join(std::process::id().to_string()),
        format!("{started}\n"),
    )
    .expect("a command that started a hundred seconds ago");

    let output = tick(&sandbox);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let record = scheduled_tick(&sandbox).expect("the tick registered itself");
    assert!(
        lease_ends_at(&record) >= now_secs() + 240,
        "the lease has to outlast the threshold the run is climbing toward: {record:?}"
    );
}

#[test]
fn a_tick_with_nothing_in_flight_lets_its_own_lease_lapse() {
    // THE OTHER DIRECTION, and it is what keeps the renewal above from being a
    // job that reschedules itself forever: an idle machine's tick has to lapse,
    // or the daemon runs it three times a minute for the life of the host over
    // a house that is holding nothing.
    let sandbox = Sandbox::new("lights-tick-lapses");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"{DEAD_BRIDGE}\"\nkey = \"k\"\n{STUDIO_MAP}"
    ));
    let output = tick(&sandbox);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        scheduled_tick(&sandbox),
        None,
        "a tick with nothing to watch registered itself again anyway"
    );
}

#[test]
fn a_lease_taken_by_hand_schedules_the_tick_that_reads_it() {
    // A LEASE NOBODY READS IS A LAMP THAT NEVER LIGHTS. `pns loop begin` is for
    // work whose length nothing can measure in advance, which is exactly the
    // run that then goes quiet: the tick's lease is refreshed by event traffic,
    // so an overnight loop taken by hand in a pane that stops talking expired
    // minutes into the run it was taken for.
    let sandbox = Sandbox::new("loop-begin-schedules-the-tick");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"{DEAD_BRIDGE}\"\nkey = \"k\"\n{STUDIO_MAP}"
    ));
    let taken = run(sandbox
        .pns_stateful()
        .args(["loop", "begin", "--pane", "wW:p21"]));
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    let record = scheduled_tick(&sandbox).expect("the lease registered the tick that reads it");
    assert!(
        lease_ends_at(&record) >= now_secs() + 3_000,
        "the tick has to outlast the lease it is there to read: {record:?}"
    );
}
