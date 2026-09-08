use super::*;

#[test]
fn a_bare_lights_command_is_a_usage_error_rather_than_an_event() {
    // FALLING THROUGH TO THE EVENT PATH IS THE FAILURE THIS PREVENTS: argv
    // parsing is deliberately lenient, so `pns lights` would otherwise skip
    // both words and fire a notification about an empty event.
    let sandbox = Sandbox::new("lights-usage");
    for argv in [vec!["lights"], vec!["lights", "wobble"]] {
        let output = logged_event(&sandbox)
            .args(&argv)
            .output()
            .expect("the engine runs");
        assert_eq!(output.status.code(), Some(2), "{argv:?}");
        assert!(
            stderr(&output).contains("usage: pns lights tick"),
            "{argv:?}: {}",
            stderr(&output)
        );
        assert!(stdout(&output).is_empty(), "{argv:?}");
        assert!(
            !sandbox.fired("hermes"),
            "{argv:?} must not become a notification"
        );
    }
}

#[test]
fn the_tick_says_nothing_at_all_however_many_times_it_runs() {
    // THE NO-CHATTER RULE, and the failure it prevents is not hypothetical: a
    // tick that traced itself would pass every other test here and then fill
    // the log that the rotate-logs job rotates a real log out of. At the
    // production refresh this runs three times a minute forever.
    let sandbox = Sandbox::new("lights-tick-quiet");
    // BOTH REPORTING LEGS ARE ENABLED, so "reaches no channel" is asserted
    // against a config where an event really would reach one.
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{}\"\nkey = \"k\"\n\
         [plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.hermes]\nenabled = true\n{STUDIO_MAP}",
        closed_port()
    ));
    plant_waiting_session(&sandbox);
    for run in 0..5 {
        let output = tick(&sandbox);
        assert_eq!(output.status.code(), Some(0), "run {run}");
        assert!(stdout(&output).is_empty(), "run {run}: {}", stdout(&output));
        assert!(stderr(&output).is_empty(), "run {run}: {}", stderr(&output));
        assert!(
            !sandbox.fired("hermes") && !sandbox.fired("mobile"),
            "run {run}: a tick is not an event and reaches no channel"
        );
    }
}

#[test]
fn the_tick_exits_zero_with_no_config_no_table_hue_off_and_an_unreachable_bridge() {
    // FOUR FAIL-OPEN DIRECTIONS IN ONE CASE, because they share an assertion
    // and a reviewer needs to see them together: the tick runs on a schedule
    // nobody is watching, and every one of these is a machine that has simply
    // not asked for the lamps yet.
    let unreachable = format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{}\"\nkey = \"k\"\n{STUDIO_MAP}",
        closed_port()
    );
    for (name, config) in [
        ("lights-tick-no-config", None),
        (
            "lights-tick-no-table",
            Some("[plugins.hue]\nenabled = true\n".to_string()),
        ),
        (
            "lights-tick-hue-off",
            Some(format!("[plugins.hue]\nenabled = false\n{STUDIO_MAP}")),
        ),
        ("lights-tick-bridge-down", Some(unreachable)),
    ] {
        let sandbox = Sandbox::new(name);
        if let Some(config) = config {
            sandbox.write_config(&config);
        }
        plant_waiting_session(&sandbox);
        let output = tick(&sandbox);
        assert_eq!(output.status.code(), Some(0), "{name}");
        assert!(stdout(&output).is_empty(), "{name}: {}", stdout(&output));
        assert!(stderr(&output).is_empty(), "{name}: {}", stderr(&output));
        assert!(
            !sandbox.fired("hermes") && !sandbox.fired("mobile"),
            "{name}: a tick is not an event and reaches no channel"
        );
    }
}

#[test]
fn the_operators_return_puts_out_a_glow_without_any_daemon_running() {
    // THE FIRST OF THE TWO CLEARS the steady glow write is paid for. The write
    // does not expire on its own, so something has to put it out, and the
    // return moment is where the condition behind it stops being true. NO
    // DAEMON IS INVOLVED: this is the event path, reading the paths it was
    // told were held and writing one PUT each.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("lights-held-cleared-on-return");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         [plugins.hermes]\nenabled = true\n{STUDIO_MAP}"
    ));
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .remember_held(&[pns_domain::lights::phase::HeldEntry::bare("light/9d52d98c")])
        .expect("a held glow");

    let mut command = logged_event(&sandbox);
    // AT THE DESK, which is what makes this event the operator's return. An
    // event that finds them away proves nothing about whether they have seen
    // the news the lamp is glowing about.
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    let child = command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let dialled = dialled_within(&listener, std::time::Duration::from_secs(5));
    let output = child.wait_with_output().expect("the child is waitable");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        dialled,
        "the return reached the bridge to put the lamp out: {}",
        stderr(&output)
    );
    assert!(
        quiet_records::held(&sandbox).is_empty(),
        "and forgot what it was holding, so the next return costs no write at all"
    );
}

#[test]
fn an_event_holding_no_glow_reaches_the_bridge_for_nothing() {
    // THE FENCE ON THE CLEAR ABOVE: the ordinary event, which is every event,
    // must not pay a bridge round trip for a lamp nobody is holding. The only
    // reading it takes is whether the file is there.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("lights-held-nothing-held");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         [plugins.hermes]\nenabled = true\n{STUDIO_MAP}"
    ));
    let mut command = logged_event(&sandbox);
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(
        !dialled_within(&listener, std::time::Duration::ZERO),
        "an ordinary event with nothing held reaches no bridge"
    );
}

#[test]
fn switching_the_lamps_off_puts_out_a_held_glow_and_switching_hue_off_keeps_the_record() {
    // THE STEADY WRITE OUTLIVES THE FEATURE THAT MADE IT, which is the whole
    // problem: it does not expire, so an operator who removes `[lights]` while
    // a lamp is glowing has nothing left that will ever put it out. The tick is
    // the only process that could, and it used to return before it read the
    // record at all.
    //
    // THE LINE IS WHETHER A BRIDGE CAN STILL BE NAMED. Hue enabled with its
    // credentials is a machine that can still address the lamp, so the tick
    // clears and forgets. Hue switched off is a machine that cannot, and a
    // record dropped there would be the same orphan through a different door.
    let held_after = |name: &str, config: &dyn Fn(u16) -> String, expect_dial: bool| {
        let (listener, port) = bridge_spy();
        let sandbox = Sandbox::new(name);
        sandbox.write_config(&config(port));
        std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
        pns_adapters::SqliteStore::for_records(sandbox.state())
            .remember_held(&[pns_domain::lights::phase::HeldEntry::bare("light/9d52d98c")])
            .expect("a held glow");
        let mut command = logged_event(&sandbox);
        sandbox.stub_herdr(&mut command, false);
        let child = command
            .args(["lights", "tick"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the engine starts");
        // THE TRUE ARM DIALS WHILE THE CHILD RUNS, same reason as everywhere
        // else that expects a dial: the spy hangs up the moment it accepts,
        // so the handshake fails at once instead of waiting out the bridge
        // deadline. THE FALSE ARM WAITS FOR THE CHILD FIRST, then takes its
        // one non-blocking accept: dialling here while the child still runs
        // had a 200 ms window where a dial arriving just after it gave up
        // would read as "no dial" that never actually happened to look.
        let dialled_while_running =
            expect_dial.then(|| dialled_within(&listener, std::time::Duration::from_secs(5)));
        let output = child.wait_with_output().expect("the child is waitable");
        assert_eq!(output.status.code(), Some(0), "{name}");
        assert!(stdout(&output).is_empty(), "{name}: {}", stdout(&output));
        let dialled = dialled_while_running
            .unwrap_or_else(|| dialled_within(&listener, std::time::Duration::ZERO));
        assert!(stderr(&output).is_empty(), "{name}: {}", stderr(&output));
        (dialled, !quiet_records::held(&sandbox).is_empty())
    };

    assert_eq!(
        held_after(
            "lights-feature-off-clears",
            &|port| format!(
                "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n"
            ),
            true,
        ),
        (true, false),
        "the map is gone but the bridge is still named, so the lamp is put out \
         by name and the record is forgotten"
    );
    assert_eq!(
        held_after(
            "lights-hue-off-keeps",
            &|port| format!(
                "[plugins.hue]\nenabled = false\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
                 {STUDIO_MAP}"
            ),
            false,
        ),
        (false, true),
        "hue switched off reaches no bridge and keeps the record, so the tick \
         after the switch goes back on still has a name to write the clear to"
    );
}

#[test]
fn a_tick_with_nothing_left_to_show_puts_out_the_glow_it_was_holding() {
    // THE SECOND OF THE TWO CLEARS, through the real tick rather than through
    // the walk's own unit test: nothing is working, nothing is unseen and no
    // session is waiting, so the house is dark and the lamp a steady write is
    // still holding has to be put out by name.
    //
    // A GUARD ADDED BY THE MUTATION TABLE, which found that no test failed
    // when the tick stopped clearing what it held.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("lights-tick-clears-its-glow");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n{STUDIO_MAP}"
    ));
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .remember_held(&[pns_domain::lights::phase::HeldEntry::bare("light/9d52d98c")])
        .expect("a held glow");
    // ACCEPTED WHILE THE TICK IS STILL RUNNING, which is what keeps this fast:
    // the spy hangs up the moment it accepts, so the TLS handshake fails at
    // once instead of sitting in the backlog for the ten-second bridge
    // deadline.
    let mut command = logged_event(&sandbox);
    sandbox.stub_herdr(&mut command, false);
    let child = command
        .args(["lights", "tick"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let dialled = dialled_within(&listener, std::time::Duration::from_secs(5));
    let output = child.wait_with_output().expect("the child is waitable");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        dialled,
        "the tick reached the bridge to put the lamp out: {}",
        stderr(&output)
    );
    assert!(
        quiet_records::held(&sandbox).is_empty(),
        "and stopped claiming to hold it"
    );
}
