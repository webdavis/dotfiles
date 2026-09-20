use super::*;

#[test]
fn a_lights_table_changes_nothing_about_an_ordinary_notification() {
    // A GUARD, not a red-first test, and it says what it still covers rather
    // than what it once did. With a `[lights]` table an ordinary long-running
    // notification no longer takes the plain room pulse at all: it resolves the
    // map on the bridge and writes per lamp, which is one or two GETs and a PUT
    // each where it used to be one group PUT. So "nothing moves" is no longer
    // true of the wire.
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
            "[plugins.lights]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\ncertificate = \"sha256:0000000000000000000000000000000000000000000000000000000000000001\"\n\
             [plugins.phone]\nenabled = true\ntype = \"moshi\"\n\
             [plugins.log]\nenabled = true\ntype = \"hermes\"\n{lights}"
        ));
        let mut command = sandbox.pns();
        // POINTS NOWHERE, as it does in every binary case here: the operator's
        // own moshi daemon is a real program on this machine and no test may
        // reach it.
        command.env("PNS_MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
        sandbox.stub_herdr(&mut command, false);
        // ACCEPTED WHILE THE CHILD IS STILL RUNNING, which is what keeps this
        // fast: the spy hangs up the moment it accepts, so the engine's TLS
        // handshake fails at once instead of waiting out the ten-second bridge
        // deadline on a connection nobody answered.
        let child = command
            .args([
                "send",
                "--producer",
                "claude",
                "--state",
                "done",
                "--detail",
                "x",
            ])
            .args(["--pane", "t1:p2", "--elapsed", "300s"])
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
            ["phone", "hermes", "banner"].map(|leg| sandbox.fired(leg)),
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
            "[lights]\narm_interval = \"20s\"\n\
             [lights.room.\"3F - Studio\"]\nbehaviours = [\"done\", \"failed\"]\n",
        ),
        "same stdout, same stderr, same exit code, the bridge dialled either way, \
         and the same legs fired"
    );
}

#[test]
fn a_bare_lights_mute_reads_the_house_dim_window_in_the_childs_own_zone() {
    // THE ONE TEST PROVING THE ZONE WIRING, and the bare mute is where the
    // window's end minute is OBSERVABLE through the binary: it is the length
    // the command reports back. The window is built from Tokyo time and ends
    // two hours from the child's own "now", so a child reading the HOST's zone
    // (or UTC, on a runner that has no other) would report a length nine hours
    // away from this one.
    let tokyo_now = (utc_minute_now() + TOKYO_MINUTES_AHEAD) % 1440;
    let (_listener, port) = bridge_spy();
    let sandbox = Sandbox::new("dim-window-bare-mute-zone");
    sandbox.write_config(&format!(
        "{}[lights.room.\"3F - Studio\"]\nbehaviours = [\"done\"]\n",
        hue_config(port, &window_around(tokyo_now, 120))
    ));
    let mut command = sandbox.bare();
    command.env("TZ", "Asia/Tokyo");
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    let output = run(command.args(["lights", "mute", "3F - Studio"]));
    let said = stdout(&output);
    let reported: u64 = said
        .lines()
        .find_map(|line| {
            let (_, rest) = line.split_once("muted for another ")?;
            rest.split_whitespace().next()?.parse().ok()
        })
        .unwrap_or_else(|| panic!("a reported length: {said}"));
    assert!(
        (118..=120).contains(&reported),
        "two hours of the child's OWN clock, got {reported} minutes: {said}"
    );
}

#[test]
fn a_bare_lights_mute_with_no_house_dim_window_is_refused_and_sets_nothing() {
    // NO SCHEDULE IS A REFUSAL, never a guessed length, and the refusal names
    // the key to write.
    let (_listener, port) = bridge_spy();
    let sandbox = Sandbox::new("dim-window-bare-mute-unset");
    sandbox.write_config(&format!(
        "[plugins.lights]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\ncertificate = \"sha256:0000000000000000000000000000000000000000000000000000000000000001\"\n\
         [lights]\narm_interval = \"20s\"\n\
         [lights.room.\"3F - Studio\"]\nbehaviours = [\"done\"]\n"
    ));
    let mut command = sandbox.bare();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    let output = run_expecting(2, command.args(["lights", "mute", "3F - Studio"]));
    let said = stderr(&output);
    assert!(
        said.contains("`[lights] dim_window` states none"),
        "the refusal names the key to write: {said}"
    );
}

#[test]
fn the_hand_run_pulse_reaches_the_bridge_whatever_the_hour() {
    // The drill is EXEMPT, structurally: `pns lights pulse` is the
    // bridge-and-key check rather than a feature, so no window gates it.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("dim-window-manual-pulse");
    sandbox.write_config(&hue_config(port, &window_around(utc_minute_now(), 120)));
    let mut command = sandbox.bare();
    command.env("TZ", "UTC");
    let child = command
        .args(["lights", "pulse", "0"])
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
