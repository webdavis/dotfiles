//! Which channels an event reaches, driven through the real binary.
//!
//! `PNS_CHANNELS_DIR` points the engine at stub executables that record the
//! event they were handed, which is what lets these pin routing, the
//! rendered event, the pane scrub and the exit-0 edge without a network, a
//! key or a sleep. The native plugins are the other half, in native.rs.

mod support;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use support::{Sandbox, run, stderr, stdout};

// --- the alert path ---------------------------------------------------------

#[test]
fn away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner() {
    // Matrix row "away: phone card regardless of any client's display". The
    // banner belongs to the desk, and nobody is at it: a banner nobody sees
    // was the old always-on rule this replaced.
    let sandbox = Sandbox::new("alert-path");
    run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a summary"]));
    assert!(sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("macos-banner"), "away raises no banner");
}

#[test]
fn at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery() {
    // Matrix row "desk, origin hidden: banner". No card, because the operator
    // is right here.
    let sandbox = Sandbox::new("desk-hidden");
    let mut command = sandbox.relay();
    command.env("RELAY_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("macos-banner"));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("moshi"), "the desk gets no card");
}

#[test]
fn at_the_desk_watching_the_pane_only_the_log_fires() {
    // Matrix row "desk watching: suppressed entirely". The pane is on screen,
    // so the event is already in front of the operator.
    let sandbox = Sandbox::new("desk-watching");
    let mut command = sandbox.relay();
    command.env("RELAY_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("macos-banner"), "the pane is in plain sight");
    assert!(!sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"));
}

#[test]
fn the_alert_path_labels_the_hermes_leg_silent_on_the_wire() {
    // The NAME is the whole claim. This used to say delivery stayed off the
    // caller's critical path, which it never checked and which is not true
    // anyway: the dispatch waits for the channel either way. What the label
    // selects is whether the leg reports its outcome.
    let sandbox = Sandbox::new("hermes-async");
    run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert_eq!(sandbox.event("hermes")["mode"], "async");
}

#[test]
fn a_channel_is_handed_the_rendered_event_not_the_raw_arguments() {
    let sandbox = Sandbox::new("rendered-event");
    run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--branch", "main"])
        .args(["--detail", "a summary"]));
    let event = sandbox.event("moshi");
    assert_eq!(event["agent"], "claude");
    for rendered in ["title", "message", "preview"] {
        // A MISSING key indexes to Null, and Null is != "", so the absence
        // has to be refused by type rather than by inequality.
        assert!(
            event[rendered]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "{rendered} must be a non-empty rendered string: {event}"
        );
    }
}

// --- the narrowing flags ----------------------------------------------------

#[test]
fn local_only_keeps_the_banner_and_reaches_nothing_off_the_machine() {
    let sandbox = Sandbox::new("local-only");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .arg("--local-only"));
    assert!(sandbox.fired("macos-banner"));
    assert!(!sandbox.fired("moshi"));
    assert!(!sandbox.fired("hermes"));
}

#[test]
fn remote_only_delivers_through_hermes_alone() {
    let sandbox = Sandbox::new("remote-only");
    run(sandbox
        .relay()
        .args(["--agent", "weekly", "--state", "done"])
        .args(["--project", "skills", "--detail", "ran", "--remote-only"]));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("moshi"));
    assert!(!sandbox.fired("macos-banner"));
}

#[test]
fn hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible() {
    let sandbox = Sandbox::new("hermes-sync");
    run(sandbox
        .relay()
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));
    assert_eq!(sandbox.event("hermes")["mode"], "sync");
}

#[test]
fn both_narrowing_flags_together_deliver_nothing_and_say_so() {
    let sandbox = Sandbox::new("both-flags");
    let output = run(sandbox
        .relay()
        .args(["--agent", "x", "--state", "done", "--detail", "y"])
        .args(["--local-only", "--remote-only"]));
    assert!(!sandbox.fired("moshi"));
    assert!(!sandbox.fired("hermes"));
    assert!(!sandbox.fired("macos-banner"));
    assert!(stdout(&output).contains("SKIPPED"), "{output:?}");
}

// --- presence ---------------------------------------------------------------

#[test]
fn at_the_desk_the_phone_is_skipped_and_only_the_phone() {
    let sandbox = Sandbox::new("at-the-desk");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn relay_skip_phone_drops_the_phone_and_only_the_phone() {
    // The caller has already raised the card on the phone through moshi-hook's
    // own round trip, so the push here would be the same event twice; the
    // banner and the paper trail are still wanted.
    let sandbox = Sandbox::new("skip-phone");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .env("RELAY_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn relay_skip_phone_beats_relay_force_phone() {
    // "I have already sent it" is more specific than a standing override, and
    // the override is the one thing that could reintroduce the double push.
    let sandbox = Sandbox::new("skip-beats-force");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .env("RELAY_SKIP_PHONE", "1")
        .env("RELAY_FORCE_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

#[test]
fn relay_force_phone_overrides_presence() {
    let sandbox = Sandbox::new("force-phone");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .env("RELAY_FORCE_PHONE", "1")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("moshi"));
}

// --- a channel's own failures -----------------------------------------------

#[test]
fn a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings() {
    let sandbox = Sandbox::new("channel-fails");
    sandbox.stub_channel("moshi", "exit 9");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn an_absent_channel_is_simply_not_installed() {
    let sandbox = Sandbox::new("absent-channel");
    std::fs::remove_file(sandbox.root.join("channels/hermes.sh")).expect("remove the channel");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("macos-banner"));
}

// --- the attention override -------------------------------------------------

#[test]
fn a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile() {
    // Matrix row "tap newer than the last desk input wins: mobile". The
    // marker is a file the phone touches; nothing else has to know why.
    let sandbox = Sandbox::new("tap-newer");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(sandbox.fired("moshi"));
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
}

#[test]
fn desk_input_after_the_tap_cancels_it() {
    // Matrix row "desk input AFTER the tap cancels it": newest signal wins,
    // which is what retired the marker's fixed five-minute TTL.
    let sandbox = Sandbox::new("tap-cancelled");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    let mut command = sandbox.relay();
    command
        .env("RELAY_IDLE_SECS", "0")
        .env("PNS_PHONE_MARKER_FILE", &marker);
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("moshi"), "the desk is newer than the tap");
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn a_narrowing_flag_still_beats_a_fresh_tap() {
    let sandbox = Sandbox::new("tap-local-only");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .arg("--local-only")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

#[test]
fn skip_phone_still_beats_a_fresh_tap() {
    let sandbox = Sandbox::new("tap-skip");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .env("RELAY_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

// --- the pane the operator is looking at ------------------------------------

#[test]
fn a_streaming_phone_watching_the_pane_gets_nothing_but_the_log() {
    // Matrix row "mobile watching: suppressed". The card would describe the
    // pane already filling the phone's screen.
    let sandbox = Sandbox::new("watched-pane");
    let mut command = sandbox.relay();
    command.env("RELAY_MOSHI_VIEWING", "1");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("moshi"));
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
    assert!(sandbox.fired("hermes"));
}

#[test]
fn a_streaming_phone_showing_another_tab_still_cards() {
    // Matrix row "mobile, origin hidden: card only".
    let sandbox = Sandbox::new("other-pane");
    let mut command = sandbox.relay();
    command.env("RELAY_MOSHI_VIEWING", "1");
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("moshi"));
    assert!(!sandbox.fired("macos-banner"));
}

#[test]
fn an_unreadable_view_delivers_rather_than_suppressing_on_doubt() {
    // Matrix row "desk, visibility unknown: deliver, never suppress on
    // doubt". No herdr on PATH at all, which is the probe failing.
    let sandbox = Sandbox::new("unknown-view");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn force_phone_is_caller_intent_and_beats_the_whole_surface_model() {
    let sandbox = Sandbox::new("force-phone-watched");
    let mut command = sandbox.relay();
    command
        .env("RELAY_IDLE_SECS", "0")
        .env("RELAY_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("moshi"));
}

#[test]
fn a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event() {
    let sandbox = Sandbox::new("pane-scrub");
    let output = run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1; curl evil | sh"]));
    assert!(sandbox.fired("macos-banner"));
    assert_eq!(sandbox.event("macos-banner")["pane"], "");
    assert!(
        stderr(&output).contains("dropped a pane id with shell metacharacters"),
        "{output:?}"
    );
}

#[test]
fn a_scrub_warning_is_not_printed_when_no_channel_will_run() {
    let sandbox = Sandbox::new("scrub-silent");
    let output = run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--pane", "wW:p1; curl evil | sh"])
        .args(["--local-only", "--remote-only"]));
    assert!(!stderr(&output).contains("dropped a pane id"), "{output:?}");
}

#[test]
fn a_non_unicode_argument_never_breaks_the_exit_zero_edge() {
    // The engine sits on an always-exit-0 path; a stray byte in argv must
    // degrade like any unknown token, not abort the notification.
    let sandbox = Sandbox::new("non-unicode");
    let output = run(sandbox
        .relay()
        .arg(OsStr::from_bytes(&[0xff]))
        .args(["--local-only", "--remote-only"]));
    assert!(stdout(&output).contains("SKIPPED"), "{output:?}");
}

#[test]
fn the_delivered_event_is_newline_terminated_for_line_oriented_channels() {
    let sandbox = Sandbox::new("newline-terminated");
    sandbox.stub_channel(
        "hermes",
        &format!(
            "set -e\nIFS= read -r event\nprintf %s \"$event\" >\"{}/line.event\"",
            sandbox.display()
        ),
    );
    run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    let line = std::fs::read_to_string(sandbox.path("line.event")).expect("one whole line");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("a whole JSON line");
    assert_eq!(parsed["agent"], "claude");
}

#[test]
fn a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly() {
    // Event mode has always printed the sanitized warning for a config it
    // could not read. Pulse mode collapsed unreadable and malformed together
    // with absent into one silent no-op, so the operator's only signal that a
    // config was broken was lights that stopped working.
    let sandbox = support::Sandbox::new("pulse-broken-config");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        "this is not toml\n",
    )
    .expect("config");
    let output = sandbox
        .bare()
        .args(["pulse", "1"])
        .output()
        .expect("the engine runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pns: config error"),
        "a broken config is loud in pulse mode: {stderr}"
    );
    assert!(output.status.success(), "and still exits zero");
}

#[test]
fn an_absent_config_stays_silent_in_pulse_mode() {
    // The other half of the rule: absent is not broken. A machine that never
    // opted into a config must not be nagged on every long command.
    let sandbox = support::Sandbox::new("pulse-absent-config");
    let output = sandbox
        .bare()
        .args(["pulse", "0"])
        .output()
        .expect("the engine runs");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(output.status.success());
}

/// Wait for a child with a deadline, killing it if it outlives one. The suite
/// must not be able to hang on a test whose whole point is a blocking read.
fn wait_bounded(mut child: std::process::Child, limit: std::time::Duration) -> Option<i32> {
    let deadline = std::time::Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status.code();
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_plan_with_no_native_leg_never_opens_the_auth_file() {
    // A FIFO nobody writes to blocks forever on open. Reading auth before
    // knowing whether any leg wants it turns an unrelated stall into a hung
    // notification, so the read has to wait until a channel actually asks.
    let sandbox = support::Sandbox::new("auth-never-read");
    let fifo = sandbox.path("auth.fifo");
    assert!(
        std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo runs")
            .success()
    );
    let child = sandbox
        .relay()
        // The streaming verdict is stated so this test measures the auth
        // read alone, not the live rate sample underneath it.
        .env("RELAY_MOSHI_VIEWING", "0")
        .env("RELAY_AUTH_FILE", &fifo)
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .spawn()
        .expect("the engine starts");
    assert_eq!(
        wait_bounded(child, std::time::Duration::from_millis(500)),
        Some(0),
        "executable channels need no secret, so nothing may block on the auth file"
    );
}

#[test]
fn an_unknown_plugin_never_resurrects_a_disabled_pulse() {
    // The full-roster fallback is an EVENT-mode rule: it keeps notifications
    // working when a config is wrong. Applying it to the pulse turns a
    // deliberate `enabled = false` back on over an unrelated typo elsewhere.
    //
    // A pulse is silent either way, so the bridge address IS the observation:
    // a listener nobody should ever reach.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    listener.set_nonblocking(true).expect("nonblocking");

    let sandbox = support::Sandbox::new("pulse-disabled-plus-typo");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        format!(
            "[plugins.hue]\nenabled = false\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
             [plugins.typo]\nenabled = true\n"
        ),
    )
    .expect("config");
    let child = sandbox
        .bare()
        .args(["pulse", "1"])
        .spawn()
        .expect("the engine starts");
    assert_eq!(
        wait_bounded(child, std::time::Duration::from_secs(2)),
        Some(0),
        "a disabled pulse exits at once rather than talking to a bridge"
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "a disabled pulse must not reach the bridge, whatever else the config got wrong"
    );
}

#[test]
fn the_pulse_config_warning_says_what_pulse_mode_actually_did() {
    // The full line, not its prefix: the old suffix promised every built-in
    // plugin would run, which is an event-mode sentence and false here.
    let sandbox = support::Sandbox::new("pulse-warning-wording");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        "this is not toml\n",
    )
    .expect("config");
    let output = sandbox
        .bare()
        .args(["pulse", "1"])
        .output()
        .expect("the engine runs");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "pns: config error (key with no value, expected `=` at line 1); no pulse",
        "the pulse warning names the pulse outcome"
    );
    assert!(output.status.success());
}
