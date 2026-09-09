use super::*;

#[test]
fn a_corrupt_lights_quiet_is_complained_about_once_rather_than_on_every_event() {
    // ONE STDERR LINE PER HOOK INVOCATION, FOREVER, is what a bare print on
    // this path buys: the file stays corrupt until a human fixes it and the
    // event path fires many times a session. The tick already routes the same
    // complaint through a remembered line, and this is that mechanism with a
    // memory of its own.
    let sandbox = Sandbox::new("lights-quiet-say-once");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"{DEAD_BRIDGE}\"\nkey = \"k\"\n\
         rooms = [\"3F - Studio\"]\nquiet_hours = \"00:00-23:59\"\n\
         [plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.hermes]\nenabled = true\n{STUDIO_MAP}"
    ));
    std::fs::create_dir_all(sandbox.state()).expect("the state directory");
    std::fs::write(sandbox.state().join("lights-quiet"), "later 3F - Studio\n")
        .expect("a state file something else wrote");
    let event = || {
        let mut command = sandbox.pns_stateful();
        command.env("TZ", "UTC");
        sandbox.stub_herdr(&mut command, false);
        stderr(&run(command.args(BLOCKED)))
    };
    let first = event();
    let second = event();
    assert!(
        first.contains("lights-quiet holds"),
        "the first event says what is wrong with the file: {first}"
    );
    assert!(
        !second.contains("lights-quiet holds"),
        "and the second says nothing, because nothing changed: {second}"
    );
}

#[test]
fn a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds() {
    // THE CALL SITE, not the rule. `record_news` and `renew_loop_lease` are
    // each pinned as functions, so this is the one test that goes red if the
    // event path stops calling them. The news record is written whatever the
    // delivery did, which is why the bridge here is dead on purpose.
    let sandbox = Sandbox::new("lights-news-and-lease");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"{DEAD_BRIDGE}\"\nkey = \"k\"\n\
         rooms = [\"3F - Studio\"]\n[plugins.hermes]\nenabled = true\n{STUDIO_MAP}"
    ));
    let lease_dir = sandbox.state().join("lights-loop");
    std::fs::create_dir_all(&lease_dir).expect("the lease directory");
    std::fs::write(lease_dir.join("t1:p2"), "500\n").expect("a lease taken by hand");
    let mut command = sandbox.pns_stateful();
    command.env("TZ", "UTC");
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    sandbox.stub_herdr(&mut command, false);
    let outcome = run(command.args(LONG_DONE));
    assert_eq!(outcome.status.code(), Some(0), "{}", stderr(&outcome));
    let news = stored_records::database(&sandbox)
        .query_row("SELECT body FROM lamp_news WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("the done event left a news record");
    let done_at: u64 = news
        .split_whitespace()
        .next()
        .and_then(|epoch| epoch.parse().ok())
        .expect("the record starts with the done epoch");
    assert!(
        done_at > 1_000_000_000,
        "a real epoch, never zero: {news:?}"
    );
    let lease = std::fs::read_to_string(lease_dir.join("t1:p2")).expect("the lease file survives");
    assert!(
        lease.trim().parse::<u64>().expect("an epoch") > 1_000_000_000,
        "the pane's own ordinary traffic moved its lease to now: {lease:?}"
    );
}

#[test]
fn the_news_record_is_written_whatever_the_lamps_are_doing() {
    // NEWS IS NOT A DELIVERY, which is what makes it independent of both lamp
    // switches. It is the record of a turn that finished or died, and the lamp
    // it later arms is what tells the operator about the ones they missed;
    // written only when a map and a transport were both live, an operator who
    // switched hue off for an evening came back to a lamp that had nothing to
    // say about the evening.
    //
    // THE TWO SWITCHES, one per case: a machine with no `[lights]` table at
    // all, and one whose map is written while the transport is off.
    for (name, config) in [
        (
            "news-without-a-map",
            "[plugins.hermes]\nenabled = true\n".to_string(),
        ),
        (
            "news-with-hue-off",
            format!(
                "[plugins.hue]\nenabled = false\n[plugins.hermes]\nenabled = true\n{STUDIO_MAP}"
            ),
        ),
    ] {
        let sandbox = Sandbox::new(name);
        sandbox.write_config(&config);
        let mut command = sandbox.pns_stateful();
        command.env("TZ", "UTC");
        command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
        sandbox.stub_herdr(&mut command, false);
        let outcome = run(command.args(LONG_DONE));
        assert_eq!(
            outcome.status.code(),
            Some(0),
            "{name}: {}",
            stderr(&outcome)
        );
        let news = stored_records::database(&sandbox)
            .query_row("SELECT body FROM lamp_news WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap_or_else(|error| panic!("{name}: no news record ({error})"));
        let done_at: u64 = news
            .split_whitespace()
            .next()
            .and_then(|epoch| epoch.parse().ok())
            .unwrap_or_else(|| panic!("{name}: the record starts with the done epoch"));
        assert!(done_at > 1_000_000_000, "{name}: a real epoch: {news:?}");
    }
}

#[test]
fn a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch() {
    // THE REPORT AND THE LAMP READ THE SAME RECORD AND DIFFERENT CLOCKS, which
    // is the one way this command can lie. The expiry is written here and read
    // by every lamp against the REAL clock, so a run that measured the mute
    // from a fixed epoch instead of from now would publish an entry already
    // expired in 1970, print "quiet for 60m" from its own arithmetic, and mute
    // nothing at all. Nothing else in the suite reads the number that lands on
    // disk, so nothing else can tell the two apart.
    let sandbox = Sandbox::new("lights-quiet-expiry-clock");
    sandbox.write_config(STUDIO_MAP);
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock")
        .as_secs();
    let muted = run(sandbox
        .pns_stateful()
        .args(["lights", "quiet", "3F - Studio", "1h"]));
    assert_eq!(muted.status.code(), Some(0), "{}", stderr(&muted));

    let published = stored_records::database(&sandbox)
        .query_row("SELECT line FROM lamp_mutes", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("the mute reached the stored row");
    let expiry: u64 = published
        .split_whitespace()
        .next()
        .and_then(|epoch| epoch.parse().ok())
        .unwrap_or_else(|| panic!("a line is `<epoch> <place>`: {published:?}"));
    // AN HOUR FROM THIS RUN, bounded on BOTH sides: a floor alone passes for
    // any clock in the future, and a ceiling alone passes for the epoch.
    assert!(
        expiry >= before + 3_600 && expiry <= before + 3_600 + 300,
        "the mute must expire an hour from now ({before}), not at {expiry}: {published:?}"
    );
}

#[test]
fn a_lights_quiet_write_that_failed_reports_the_disk_and_not_the_list_it_built() {
    // THE WORST OUTCOME THIS COMMAND HAS: telling a human a mute is in effect
    // that is not. `kept` is what the file WOULD have held, so a report printed
    // after a failed write describes a house that does not exist, and for a
    // failed `off` it says nothing is quiet while the old mute is still on disk
    // and still taking the lamp.
    let sandbox = Sandbox::new("lights-quiet-unwritable");
    sandbox.write_config(STUDIO_MAP);
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o500))
        .expect("a state directory this run cannot write");
    let refused = sandbox
        .pns_stateful()
        .args(["lights", "quiet", "3F - Studio", "1h"])
        .output()
        .expect("the engine runs");
    // RESTORED BEFORE THE ASSERTIONS, so a failure here still leaves a sandbox
    // that can be removed.
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700))
        .expect("the directory goes back");
    assert_eq!(
        refused.status.code(),
        Some(1),
        "the run failed: {}",
        stderr(&refused)
    );
    assert!(
        stderr(&refused).contains("lights-quiet could not be written"),
        "and it says so: {}",
        stderr(&refused)
    );
    assert_eq!(
        stdout(&refused),
        "",
        "and reports NOTHING, because nothing on disk changed"
    );
}
