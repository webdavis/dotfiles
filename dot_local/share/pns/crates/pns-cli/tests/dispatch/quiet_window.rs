use super::*;

#[test]
fn a_lights_table_changes_nothing_about_an_ordinary_notification() {
    // A GUARD, not a red-first test, and it says what it still covers rather
    // than what it once did. With a `[lights]` table an ordinary long-running
    // notification no longer takes the `[plugins.hue] rooms` path at all: it
    // resolves the map on the bridge and writes per lamp, which is one or two
    // GETs and a PUT each where it used to be one group PUT. So "nothing
    // moves" is no longer true of the wire.
    //
    // WHAT IT STILL PINS, and what it fails on the day one of them moves: the
    // stdout, the stderr, the exit code, the SET OF LEGS that fired, and that
    // the bridge is still reached at all. The legs are here because a dial is
    // a single boolean and a table that quietly cost the operator their card
    // would pass a test that only asked whether a bulb was addressed.
    let outcome = |name: &str, lights: &str| {
        let (listener, port) = bridge_spy();
        let sandbox = Sandbox::new(name);
        // BOTH REPORTING LEGS ARE ENABLED, so the comparison below runs
        // against a baseline where something other than a bulb is live: a
        // table that cost the operator their card would otherwise sit inside
        // a leg nobody switched on.
        sandbox.write_config(&format!(
            "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
             rooms = [\"3F - Studio\"]\n[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
             [plugins.hermes]\nenabled = true\n{lights}"
        ));
        let mut command = sandbox.pns();
        // POINTS NOWHERE, as it does in every binary case here: the operator's
        // own moshi daemon is a real program on this machine and no test may
        // reach it.
        command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
        sandbox.stub_herdr(&mut command, false);
        // ACCEPTED WHILE THE CHILD IS STILL RUNNING, which is what keeps this
        // fast: the spy hangs up the moment it accepts, so the engine's TLS
        // handshake fails at once instead of waiting out the ten-second bridge
        // deadline on a connection nobody answered.
        let child = command
            .args(["--agent", "claude", "--state", "done", "--detail", "x"])
            .args(["--pane", "t1:p2", "--long-running"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the engine starts");
        let dialled = dialled_within(&listener, std::time::Duration::from_secs(5));
        let output = child.wait_with_output().expect("the child is waitable");
        // The sandbox's own path is the one thing that legitimately differs
        // between two runs, so it is replaced rather than compared.
        let scrub = |said: String| said.replace(&sandbox.display(), "<sandbox>");
        (
            scrub(stdout(&output)),
            scrub(stderr(&output)),
            output.status.code(),
            dialled,
            // EVERY LEG THIS EVENT COULD REACH, named rather than counted, so
            // a table that swapped one destination for another cannot pass.
            ["mobile", "hermes", "macos-banner"].map(|leg| sandbox.fired(leg)),
        )
    };
    let without_a_table = outcome("lights-guard-without-a-table", "");
    assert_eq!(
        (without_a_table.3, without_a_table.4),
        (true, [true, true, false]),
        "the comparison only means something against a live baseline: this event \
         really does light the room, really does reach the phone and the durable \
         log, and really does leave the banner alone: {without_a_table:?}"
    );
    assert_eq!(
        without_a_table,
        outcome(
            "lights-guard-with-a-table",
            "[lights]\nrefresh_secs = 20\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"done\", \"failed\"]\n",
        ),
        "same stdout, same stderr, same exit code, the bridge dialled either way, \
         and the same legs fired"
    );
}

#[test]
fn a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg() {
    // The window mutes the LIGHTS and nothing else: the card and the log are
    // how a long command reports at any hour, and only the room stays dark.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-window-mutes-the-pulse");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"{}\"\n[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
         [plugins.hermes]\nenabled = true\n",
        window_around(utc_minute_now(), 120)
    ));
    let mut command = sandbox.pns();
    command.env("TZ", "UTC");
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2", "--long-running"]));
    assert!(
        sandbox.fired("mobile") && sandbox.fired("hermes"),
        "every other leg still dispatches inside the window"
    );
    assert!(
        !dialled_within(&listener, std::time::Duration::ZERO),
        "and the room stays dark"
    );
}

#[test]
fn a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due() {
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-window-malformed");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"10pm-7am\"\n[plugins.hermes]\nenabled = true\n"
    ));

    // An event that earned no pulse says nothing about the window: a refusal
    // on every notification is the noise this gate sits inside the `if` to
    // avoid.
    let mut ordinary = sandbox.pns();
    sandbox.stub_herdr(&mut ordinary, false);
    let ordinary = run(ordinary.args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        !stderr(&ordinary).contains("quiet_hours"),
        "a notification that was never going to light the room is not where a \
         window is diagnosed: {}",
        stderr(&ordinary)
    );

    let mut pulsing = sandbox.pns();
    sandbox.stub_herdr(&mut pulsing, false);
    let pulsing = run(pulsing
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2", "--long-running"]));
    let said = stderr(&pulsing);
    assert_eq!(
        said.matches("hue.quiet_hours").count(),
        1,
        "one refusal, naming the key: {said}"
    );
    assert!(
        said.contains("10pm-7am"),
        "and echoing what was written: {said}"
    );
    assert!(
        !dialled_within(&listener, std::time::Duration::ZERO),
        "a window nobody can parse leaves the room dark rather than flashing it"
    );
}

#[test]
fn the_hand_run_pulse_reaches_the_bridge_inside_the_quiet_window() {
    // The drill is EXEMPT, structurally: `pns pulse` never passes the event
    // path's gate, because gating it would make the quiet window impossible to
    // check by hand exactly while it is on.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-window-manual-pulse");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"{}\"\n",
        window_around(utc_minute_now(), 120)
    ));
    let mut command = sandbox.bare();
    command.env("TZ", "UTC");
    let child = command
        .args(["pulse", "0"])
        .spawn()
        .expect("the engine starts");
    assert!(
        dialled_within(&listener, std::time::Duration::from_secs(5)),
        "the operator asked for a pulse by hand and got one"
    );
    assert_eq!(
        wait_bounded(child, std::time::Duration::from_secs(5)),
        Some(0),
        "and it still exits zero"
    );
}

#[test]
fn the_window_is_read_in_the_zone_the_child_was_given() {
    // THE ONE TEST PROVING THE ZONE WIRING. Both halves are built from Tokyo
    // time, and both are placed so that a child reading the HOST's zone (or
    // UTC, on a runner that has no other) lands on the wrong side: the quiet
    // half would dial, and the loud half, which is the twelve hours on the far
    // side of the clock, would go dark.
    let tokyo_now = (utc_minute_now() + TOKYO_MINUTES_AHEAD) % 1440;

    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-window-zone-quiet");
    sandbox.write_config(&hue_config(port, &window_around(tokyo_now, 120)));
    let mut quiet = sandbox.pns();
    quiet.env("TZ", "Asia/Tokyo");
    sandbox.stub_herdr(&mut quiet, false);
    run(quiet
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2", "--long-running"]));
    assert!(
        !dialled_within(&listener, std::time::Duration::ZERO),
        "the child is inside a window written in ITS zone, so the room stays dark"
    );

    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-window-zone-loud");
    sandbox.write_config(&hue_config(
        port,
        &window_around((tokyo_now + 720) % 1440, 360),
    ));
    let mut loud = sandbox.pns();
    loud.env("TZ", "Asia/Tokyo");
    sandbox.stub_herdr(&mut loud, false);
    let child = loud
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2", "--long-running"])
        .spawn()
        .expect("the engine starts");
    assert!(
        dialled_within(&listener, std::time::Duration::from_secs(5)),
        "and outside one it pulses"
    );
    assert_eq!(
        wait_bounded(child, std::time::Duration::from_secs(5)),
        Some(0),
        "on the exit-zero edge either way"
    );
}
