use super::*;

#[test]
fn a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it() {
    let sandbox = Sandbox::new("quiet-set");
    let output = run(mute_command(&sandbox).arg("30m"));
    assert_eq!(
        stdout(&output).trim_end(),
        "pns: muted for another 30 minutes"
    );

    // ONE ABSOLUTE EXPIRY, not a flag and not a start plus a duration: every
    // reader compares it with its own clock, so nothing has to know when the
    // mute began and a record left behind after the window is inert.
    let expiry = quiet_records::expiry(&sandbox).expect("one stored epoch second");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs();
    assert!(
        (now + 1_795..=now + 1_800).contains(&expiry),
        "thirty minutes from now, got {expiry} against {now}"
    );

    // THE NO-ARGUMENT FORM REPORTS AND MUTES NOTHING, which is what keeps any
    // invocation from muting by accident.
    //
    // A RETAINED READER OBSERVES COMMITS, not just content: a re-publish can
    // store the same expiry in the same second. SQLite's data_version changes
    // on this connection when another connection changes stored data. An
    // identical-value update can be a SQLite no-op and changes no stored row.
    let observer = stored_records::database(&sandbox);
    let published_at = modified_at(&observer);
    let again = run(&mut mute_command(&sandbox));
    assert_eq!(
        stdout(&again).trim_end(),
        "pns: muted for another 30 minutes"
    );
    assert_eq!(
        modified_at(&observer),
        published_at,
        "a report must not rewrite the mute it is reporting"
    );
}

#[test]
fn off_removes_the_state_file_and_the_next_event_decorates_again() {
    let sandbox = Sandbox::new("quiet-off");
    run(mute_command(&sandbox).arg("30m"));
    assert!(
        quiet_records::expiry(&sandbox).is_some(),
        "muted to begin with"
    );

    let output = run(mute_command(&sandbox).arg("off"));
    assert_eq!(stdout(&output).trim_end(), "pns: not muted");
    // DELETED, not overwritten with a past expiry or a flag reading off:
    // the absent quiet row is the successor to the absent legacy file.
    assert!(
        quiet_records::expiry(&sandbox).is_none(),
        "off leaves nothing behind to interpret"
    );

    let mut event = sandbox.pns();
    event.env("PNS_STATE_DIR", sandbox.path("state"));
    event.env("PNS_SCREEN_IDLE", "0");
    sandbox.stub_herdr(&mut event, false);
    run(event
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
        .args(["--pane", "t1:p2"]));
    assert!(
        sandbox.fired("banner"),
        "the banner is back the moment the mute is off"
    );
}

#[test]
fn a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge() {
    // The whole mute, end to end: a record, a clock and a subcommand, which is
    // only provable through the binary. Away and long running is the loudest
    // row in the matrix, so it is the one worth silencing.
    let away_and_long = |sandbox: &Sandbox, port: u16| {
        sandbox.write_config(&format!(
            "[plugins.lights]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\ncertificate = \"sha256:0000000000000000000000000000000000000000000000000000000000000001\"\n\
             [plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.hermes]\nenabled = true\n\
             [plugins.banner]\nenabled = true\n"
        ));
        let mut event = sandbox.pns();
        event.env("PNS_STATE_DIR", sandbox.path("state"));
        event
            .args([
                "send",
                "--producer",
                "claude",
                "--state",
                "done",
                "--detail",
                "x",
            ])
            .args(["--pane", "t1:p2", "--elapsed", "300s"]);
        event
    };

    // THE UNMUTED CONTROL, so the silence below is the mute and not a config
    // that was never going to fire anything.
    let (listener, port) = bridge_spy();
    let loud = Sandbox::new("quiet-muted-control");
    let mut command = away_and_long(&loud, port);
    let child = command.spawn().expect("the engine starts");
    assert!(
        dialled_within(&listener, std::time::Duration::from_secs(5)),
        "unmuted control: the room lights"
    );
    assert_eq!(
        wait_bounded(child, std::time::Duration::from_secs(5)),
        Some(0)
    );
    assert!(loud.fired("mobile"), "unmuted control: the phone is carded");

    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-muted-event");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .set_mute_expiry(Some(expiry))
        .expect("the mute");
    run(&mut away_and_long(&sandbox, port));

    assert!(
        sandbox.fired("hermes"),
        "THE RECORD SURVIVES THE MUTE: hermes is not a field of the delivery \
         plan, so the durable log is exempt structurally and the mute is lossless"
    );
    assert!(!sandbox.fired("mobile"), "no card while muted");
    assert!(!sandbox.fired("banner"), "no banner while muted");
    assert!(
        !dialled_within(&listener, std::time::Duration::ZERO),
        "and no pulse, so slice 7's window is never even consulted"
    );
}

#[test]
fn a_corrupt_state_file_delivers_everything_and_complains_once_per_event() {
    // THE FAIL DIRECTION, and the one a reviewer should attack first: a file
    // nobody can parse is NOT muted. Failing closed here would cost every
    // notification, including the card for a tool call the operator is blocked
    // on, with no expiry on it and nothing announcing the state.
    let sandbox = Sandbox::new("quiet-corrupt");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(quiet_state(&sandbox), "later\n").expect("a broken mute");

    let mut event = sandbox.pns();
    event.env("PNS_STATE_DIR", sandbox.path("state"));
    // At the desk with the pane out of sight, and the card forced: the one
    // event that earns BOTH decorations, so a mute reading true here would be
    // unmissable.
    event.env("PNS_SCREEN_IDLE", "0");
    event.env("PNS_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut event, false);
    let output = run(event
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
        .args(["--pane", "t1:p2"]));

    assert!(sandbox.fired("banner"), "a broken mute mutes nothing");
    assert!(sandbox.fired("mobile"), "including a forced card");
    assert!(sandbox.fired("hermes"));
    // ONE COMPLAINT PER EVENT, not one per reader: the file is broken until
    // someone fixes it, so it repeats on the next event, but a single run must
    // not say it twice.
    let complaints = stderr(&output)
        .lines()
        .filter(|line| line.starts_with("pns: state error"))
        .map(String::from)
        .collect::<Vec<_>>();
    assert_eq!(
        complaints,
        vec![
            "pns: state error (quiet-until is \"later\", not an expiry time); \
             nothing is muted, clear it with pns mute off"
        ],
        "the file's own content, and the remedy, said once: {}",
        stderr(&output)
    );
}

#[test]
fn an_absent_state_file_is_the_ordinary_state_and_says_nothing() {
    // The normal case must be silent, or the complaint becomes noise on every
    // event forever and stops being read.
    let sandbox = Sandbox::new("quiet-absent");
    let mut event = sandbox.pns();
    event.env("PNS_STATE_DIR", sandbox.path("state"));
    event.env("PNS_SCREEN_IDLE", "0");
    sandbox.stub_herdr(&mut event, false);
    let output = run(event
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("banner"));
    assert_eq!(stderr(&output), "", "no file, no news");
}

#[test]
fn quiet_calendar_arms_the_mute_through_the_argv_the_daemon_schedules() {
    // THE DAEMON NEVER CALLS `mute_calendar_mode` DIRECTLY: it schedules
    // `["mute", "calendar"]` (calendar_registration.rs) and the engine
    // dispatches on argv like every other invocation. Running the binary
    // with that exact argv is what pins the wiring between them, not a call
    // into `poll()`.
    let sandbox = Sandbox::new("quiet-calendar-dispatch");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("calendar-stub");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs();
    let end = now + 600;
    write_script(
        &script,
        &format!(
            "printf '{{\"events\":[{{\"start\":{},\"end\":{end},\"busy\":true}}]}}'",
            now.saturating_sub(60)
        ),
    );
    sandbox.write_config(&format!(
        "[quiet.calendar]\nenabled = true\ncommand = [\"{}\"]\n",
        script.display()
    ));

    let mut armed = sandbox.pns();
    armed.env("PNS_STATE_DIR", sandbox.state());
    let output = run(armed.args(["mute", "calendar"]));
    assert_eq!(
        stdout(&output).trim_end(),
        "pns: muted for a calendar event"
    );

    let report = run(&mut mute_command(&sandbox));
    let reported = stdout(&report).trim_end().to_string();
    assert!(
        reported.starts_with("pns: muted for another"),
        "the calendar's own arm reads back as a standing mute: {reported}"
    );

    let state = std::fs::read_to_string(sandbox.state().join("quiet-calendar"))
        .expect("the calendar state file");
    assert_eq!(
        state.trim_end(),
        format!("{end} 0"),
        "armed until the event's end"
    );
}

#[test]
fn a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state() {
    // A SUBCOMMAND THAT SILENTLY ACCEPTS A TYPO IS A MUTE THE OPERATOR
    // BELIEVES IS ON. This is not the always-exit-0 contract's territory: that
    // covers the hook and notification paths, where a non-zero exit would fail
    // the turn being reported on, and `pns mute` is hand typed.
    const USAGE: &str = "pns: usage: pns mute [<duration>|off|calendar]; duration is <count><s|m|h>, from 1s to 24h";
    for arguments in [
        vec!["tomorrow"],
        vec!["30"],
        vec!["off", "please"],
        vec!["30m", "extra"],
    ] {
        let sandbox = Sandbox::new("quiet-refusal");
        let output = mute_command(&sandbox)
            .args(&arguments)
            .output()
            .expect("the engine runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).lines().any(|line| line == USAGE),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), "", "arguments: {arguments:?}");
        assert!(
            !sandbox.path("state/pns.db").exists(),
            "a refused mute writes no state: {arguments:?}"
        );
    }
}

#[test]
fn the_argv_delivery_class_crosses_the_same_mute_edge_json_does() {
    // The mute-bypass check reads `event.delivery_class`, which both the
    // `--delivery-class` flag and the JSON `delivery_class` field write into,
    // but only the JSON path (`hooks/delivery_class.rs`) drove a bypass
    // assertion. This is the argv twin, so a regression on either spelling is
    // caught the same way.
    let muted = |sandbox: &Sandbox, table: &str| {
        sandbox.write_config(&format!("{}{table}", support::STUB_CHANNELS));
        std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
        pns_adapters::SqliteStore::for_records(sandbox.state())
            .set_mute_expiry(Some(i64::MAX as u64))
            .expect("the mute");
        let mut event = sandbox.pns();
        event.env("PNS_STATE_DIR", sandbox.path("state"));
        event.env("PNS_SCREEN_IDLE", "0");
        event.args([
            "send",
            "--producer",
            "claude",
            "--state",
            "blocked",
            "--detail",
            "x",
            "--delivery-class",
            "security",
        ]);
        event
    };

    let sandbox = Sandbox::new("argv-class-bypass-allowed");
    run(&mut muted(
        &sandbox,
        "[delivery_class.security]\nbypass_mute = true\n",
    ));
    assert!(
        sandbox.fired("banner"),
        "a class the config lists crosses the mute on the argv path too"
    );

    let sandbox = Sandbox::new("argv-class-bypass-refused");
    run(&mut muted(
        &sandbox,
        "[delivery_class.security]\nbypass_mute = false\n",
    ));
    assert!(
        !sandbox.fired("banner"),
        "an unlisted class stays muted on the argv path too"
    );
}

#[test]
fn a_typed_duration_in_hours_mutes_for_that_many_hours() {
    let sandbox = Sandbox::new("mute-two-hours");
    let output = run(mute_command(&sandbox).arg("2h"));
    assert_eq!(
        stdout(&output).trim_end(),
        "pns: muted for another 120 minutes"
    );
}

#[test]
fn the_word_quiet_is_refused_by_both_mutes_and_names_the_word_that_replaced_it() {
    // A RETIRED WORD THAT STILL RAN would be the one outcome worth nothing
    // here: an operator typing the old spelling has to learn the new one, and
    // a silent success teaches them nothing.
    for (arguments, sentence) in [
        (
            vec!["quiet"],
            "pns: quiet is now mute: run `pns mute <duration>`",
        ),
        (
            vec!["lights", "quiet"],
            "pns: quiet is now mute: run `pns lights mute [<place> [<duration>|off]]`",
        ),
    ] {
        let sandbox = Sandbox::new("mute-retired-word");
        let output = sandbox
            .pns_stateful()
            .args(&arguments)
            .output()
            .expect("the engine runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).lines().any(|line| line == sentence),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
    }
}
