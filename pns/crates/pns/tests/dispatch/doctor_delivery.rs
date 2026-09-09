use super::*;

#[test]
fn a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one() {
    // THE FAILING CHANNEL IS THE FIRST ONE DISPATCHED, which is the whole
    // point: failing the LAST enabled channel is a scenario a census that
    // stopped at the first failure would pass unchanged. moshi leads the
    // delivery order and has no token, the banner behind it still RECEIVES
    // its payload, and hermes at the tail still gets its turn and says so.
    //
    // NATIVE, because a stub channel is silent by design and could never
    // report a failure: leaving `PNS_CHANNELS_DIR` unset is the only condition
    // under which the compiled-in plugins win. A config with no secrets is how
    // both failures are produced end to end, with nothing stubbed to fail on
    // command.
    let sandbox = Sandbox::new("doctor-failure");
    sandbox.write_config(
        "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.macos-banner]\nenabled = true\n\
         [plugins.hermes]\nenabled = true\n",
    );
    let mut command = sandbox.bare();
    // Belt and braces: with no key nothing is posted at all, and if that ever
    // changed this points the post at a port nothing listens on rather than at
    // the operator's own gateway.
    command.env("PNS_HERMES_URL", "http://127.0.0.1:1/hook");
    sandbox.stub_notifier(&mut command);
    // BY HAND, because this one needs `bare()` to reach the native plugins and
    // so cannot go through `doctor_command`. Every doctor invocation in this
    // file has to name a moshi-hook or it runs the operator's own.
    no_moshi_hook(&sandbox, &mut command);
    let output = command.arg("doctor").output().expect("the engine runs");

    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        printed.contains(
            "mobile: FAILED, push SKIPPED -- no moshi token in the config \
             ([plugins.mobile] token); nothing was sent"
        ),
        "the first channel's own sentence, verbatim: {printed}"
    );
    assert!(
        printed.contains("macos-banner: sent, posted the banner"),
        "the leg behind the failure still delivered: {printed}"
    );
    assert!(
        sandbox.path("notifier.args").exists(),
        "and it was handed its payload, not merely reported on"
    );
    assert!(
        printed.contains(
            "hermes: FAILED, post SKIPPED -- no hermes key in the config \
             ([plugins.hermes] key); nothing was sent"
        ),
        "the last leg still got its turn after an earlier failure: {printed}"
    );
    assert!(
        printed.contains("pns doctor: 1 sent, 2 failed, 3 skipped"),
        "{printed}"
    );
}

#[test]
fn a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made() {
    // MEASURED before the fix: a channels directory with nothing in it
    // reported "3 sent, 0 failed" and exited 0, because a spawn that never
    // happened and a channel that ran and said nothing came back as the same
    // verdict. Green for a directory holding no channel at all is the one
    // answer a hand-run check must never give.
    let sandbox = Sandbox::new("doctor-unlaunchable");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let empty = sandbox.path("empty-channels");
    std::fs::create_dir_all(&empty).expect("an empty channels dir");
    let mut command = doctor_command(&sandbox);
    command.env("PNS_CHANNELS_DIR", &empty);
    let output = command.output().expect("the engine runs");

    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    for channel in ["mobile", "macos-banner", "hermes"] {
        assert!(
            printed.lines().any(|line| line.starts_with(&format!(
                "{channel}: FAILED, could not launch the channel at"
            ))),
            "{channel} was reported as sent by a spawn that never happened: {printed}"
        );
    }
    assert!(
        printed.contains("pns doctor: 0 sent, 3 failed, 3 skipped"),
        "the summary has to count what the lines say: {printed}"
    );
}

#[test]
fn the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides() {
    // THE BYPASSES THIS RUN CAN OBSERVE. A mute standing, the operator at
    // their desk, and both phone overrides set: on the event path the mute
    // strips the decoration, the desk drops the phone and skip-phone drops it
    // again over the top of force-phone, and here every channel still
    // receives. Together they are the state someone is in when they stop to
    // ask whether their channels still work.
    //
    // THE VIEWED-PANE RULE IS NOT AMONG THEM, and cannot be: `decide` is never
    // called on this path, so no pane verdict exists to bypass and this run
    // cannot tell a bypassed rule from an absent one. The session view is
    // stubbed as watching the origin pane only so that a live herdr on the
    // developer's own machine cannot decide the verdict.
    let sandbox = Sandbox::new("doctor-bypasses-the-gates");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    std::fs::write(sandbox.path("state/quiet-until"), format!("{expiry}\n")).expect("the mute");

    let mut command = doctor_command(&sandbox);
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_SKIP_PHONE", "1")
        .env("PNS_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut command, true);
    let output = command.output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    for channel in ["mobile", "macos-banner", "hermes"] {
        assert!(
            sandbox.fired(channel),
            "{channel} was suppressed by a gate the doctor exists to bypass: {}",
            stdout(&output)
        );
    }
    // The first read imports the existing mute. Doctor adds no event records.
    assert_eq!(
        std::fs::read_to_string(sandbox.path("state/quiet-until")).unwrap(),
        format!("{expiry}\n")
    );
    let stored: String = database(&sandbox)
        .query_row("SELECT body FROM quiet WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, format!("{expiry}\n"));
    assert!(
        decisions(&sandbox).is_empty(),
        "doctor recorded its own decision"
    );
    assert!(
        activity(&sandbox).is_empty(),
        "doctor recorded its own activity"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn the_doctor_reaches_the_bridge_inside_the_lights_quiet_window() {
    // The exemption `pns pulse` already has, for the same reason: gating the
    // hand-run check would make the window uncheckable exactly while it is on.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("doctor-quiet-window");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"{}\"\n",
        window_around(utc_minute_now(), 120)
    ));
    let mut command = sandbox.bare();
    command.env("TZ", "UTC");
    // BY HAND, for the same reason as above: `bare()` is what reaches the
    // native lights, and an unnamed moshi-hook is the operator's own.
    no_moshi_hook(&sandbox, &mut command);
    let child = command.arg("doctor").spawn().expect("the engine starts");
    assert!(
        dialled_within(&listener, std::time::Duration::from_secs(5)),
        "the operator asked for a check by hand and the lights were not part of it"
    );
    assert!(
        wait_bounded(child, std::time::Duration::from_secs(5)).is_some(),
        "and it finished rather than parking on the bridge"
    );
}

#[test]
fn a_pulse_with_no_bridge_to_dial_names_the_settings_rather_than_the_rooms() {
    // `fire_pulse` answers zero rooms both for a bridge that listed none and
    // for a hue table that resolves to no bridge at all, and the zero-rooms
    // line blames the listing or the room names: production-reachable
    // misdirection, sending the operator hunting through a bridge nothing
    // dialled. The spy is here to prove nothing dialled it.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("doctor-hue-unresolved");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\n"
    ));
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        printed.contains(
            "hue: FAILED, pulse SKIPPED -- no hue bridge and key in the config \
             ([plugins.hue] bridge, key); nothing was signalled"
        ),
        "the line names the settings to write: {printed}"
    );
    assert!(
        !dialled_within(&listener, std::time::Duration::ZERO),
        "a bridge was dialled for a config that resolves to none"
    );
}

#[test]
fn a_pulse_the_bridge_answered_nothing_for_still_names_both_causes_it_cannot_choose_between() {
    // THE MIRROR of the line above, and the reason the zero-rooms sentence
    // stays: a bridge and key that resolve ARE dialled, and a run that came
    // back with no room cannot tell an empty listing from a room name nothing
    // matched. Naming the settings here would send the operator to edit a
    // config that is already right.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("doctor-hue-listed-nothing");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n"
    ));
    // SPAWNED, not run to completion: the spy has to accept while the engine
    // is still dialling, or the bridge deadline is what this test waits out.
    let mut command = doctor_command(&sandbox);
    command.stdout(std::process::Stdio::piped());
    let child = command.spawn().expect("the engine starts");
    assert!(
        dialled_within(&listener, std::time::Duration::from_secs(5)),
        "a resolvable bridge was never contacted"
    );
    let output = child.wait_with_output().expect("the engine finishes");

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains(
            "hue: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)"
        ),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_config_that_enables_nothing_names_every_plugin_sends_nothing_and_exits_one() {
    let sandbox = Sandbox::new("doctor-nothing-enabled");
    sandbox.write_config("[plugins.mobile]\nenabled = false\n");
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "a check with nothing to check must never report green: {}",
        stderr(&output)
    );
    let reported = stdout(&output);
    let printed: Vec<&str> = reported.lines().skip(1).collect();
    assert_eq!(
        printed,
        [
            "router: skipped, not enabled in the config",
            "presence: skipped, not enabled in the config",
            "mobile: skipped, not enabled in the config",
            "macos-banner: skipped, not enabled in the config",
            "hermes: skipped, not enabled in the config",
            "hue: skipped, not enabled in the config",
            "pns doctor: 0 sent, 0 failed, 6 skipped",
            NO_MOSHI_HOOK_LINE,
            FOCUS_OFF_LINE,
            DAEMON_NEVER_RAN_LINE,
            NAG_OFF_LINE,
            LIGHTS_OFF_LINE,
            "pns doctor: delivery ledger unreadable; backlog and deadletters unknown",
            // An empty ledger is an empty roster: pns learns a route only by
            // having posted to one, so this says nothing has been posted yet
            // rather than reporting a clean bill of health.
            "pns doctor: no routes to check; nothing has been posted yet",
            NO_DECISION_RECORDED,
            NONE_WAITING,
        ],
        "the whole roster is still the report; only a census can say this"
    );
    for channel in ["mobile", "macos-banner", "hermes"] {
        assert!(
            !sandbox.fired(channel),
            "{channel} received a payload from a config that enabled nothing"
        );
    }
}
