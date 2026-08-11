//! The native plugins: the branch no stub channel can reach.
//!
//! Leaving `PNS_CHANNELS_DIR` unset is the only condition under which the
//! compiled-in plugins win, so these stub `terminal-notifier` on PATH and
//! point the channel URLs at the crate's one-shot capture binary instead.
//! std only on both sides, so there is no interpreter cold start to
//! diagnose when one fails.

mod support;

use hmac::{Hmac, KeyInit, Mac};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use support::{CAPTURE, Sandbox, run, stderr, stdout, write_script};

/// The capture server, already bound to its ephemeral port.
struct Capture {
    server: Child,
    port: u16,
    captured: PathBuf,
}

impl Capture {
    fn start(sandbox: &Sandbox, name: &str, status: Option<&str>) -> Self {
        let port_file = sandbox.path(&format!("{name}.port"));
        let captured = sandbox.path(&format!("{name}.capture"));
        let mut command = Command::new(CAPTURE);
        command.arg(&port_file).arg(&captured);
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
            captured,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The raw request, once the server has answered and exited.
    fn finish(mut self) -> String {
        let _ = self.server.wait();
        std::fs::read_to_string(&self.captured).unwrap_or_default()
    }
}

/// Everything past the blank line that ends the headers.
fn body_of(raw: &str) -> &str {
    let after_headers = raw
        .find("\r\n\r\n")
        .map(|at| at + 4)
        .or_else(|| raw.find("\n\n").map(|at| at + 2));
    &raw[after_headers.expect("a header terminator")..]
}

fn header_of(raw: &str, name: &str) -> Option<String> {
    raw.lines()
        .find(|line| line.to_ascii_lowercase().starts_with(&format!("{name}: ")))
        .map(|line| line[name.len() + 2..].trim().to_string())
}

/// HMAC-SHA256 in hex, computed here rather than through the crate's own
/// signer: an independent implementation is what makes this catch a broken
/// `sign`, the way the retired gate used openssl.
fn expected_signature(key: &str, body: &str) -> String {
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key.as_bytes()).expect("any key length");
    mac.update(body.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn the_banner_leg_delivers_natively_and_the_executable_channel_stays_silent() {
    let sandbox = Sandbox::new("native-banner");
    let decoy = sandbox.path(".local/libexec/pns/channels");
    std::fs::create_dir_all(&decoy).expect("decoy dir");
    write_script(
        &decoy.join("macos-banner.sh"),
        &format!("cat >\"{}/decoy.event\"", sandbox.display()),
    );

    let mut command = sandbox.bare();
    command.env("RELAY_IDLE_SECS", "99999");
    sandbox.stub_notifier(&mut command);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .arg("--local-only"));

    let spawned = std::fs::read_to_string(sandbox.path("notifier.args"))
        .expect("the native banner spawns the notifier");
    assert!(spawned.contains("-title"), "{spawned}");
    assert!(
        !sandbox.path("decoy.event").exists(),
        "the decoy executable channel fired; native did not win"
    );
}

#[test]
fn native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output() {
    let sandbox = Sandbox::new("native-moshi");
    let auth = sandbox.write_auth("{\"moshi_secret\":\"tok-integration\"}\n");
    let capture = Capture::start(&sandbox, "moshi", None);

    let mut command = sandbox.bare();
    command
        .env("RELAY_IDLE_SECS", "99999")
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_MOSHI_URL", capture.url());
    sandbox.stub_notifier(&mut command);
    let output = run(command.args(["--agent", "claude", "--state", "done", "--detail", "x"]));

    let raw = capture.finish();
    assert!(
        header_of(&raw, "content-type")
            .is_some_and(|value| value.eq_ignore_ascii_case("application/json")),
        "the request carried no JSON content type: {raw}"
    );
    let body: serde_json::Value =
        serde_json::from_str(body_of(&raw).trim()).expect("the posted body is JSON");
    assert_eq!(body["token"], "tok-integration");
    assert_eq!(body["title"], "claude · done");

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
    let auth = sandbox.write_auth("{\"moshi_secret\":\"tok-integration\"}\n");
    let mut command = sandbox.bare();
    command
        .env("RELAY_IDLE_SECS", "99999")
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_MOSHI_URL", "http://127.0.0.1:1");
    sandbox.stub_notifier(&mut command);
    let output = run(command.args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        stderr(&output).is_empty(),
        "a failed post said something; the failure path must be silent: {output:?}"
    );
}

#[test]
fn sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent() {
    let sandbox = Sandbox::new("native-hermes");
    let auth = sandbox.write_auth("{\"hermes_secret\":\"gate-signing-key\"}\n");
    let capture = Capture::start(&sandbox, "hermes", None);

    let mut command = sandbox.bare();
    command
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_HERMES_URL", capture.url());
    sandbox.stub_notifier(&mut command);
    let output = run(command
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));

    assert_eq!(stdout(&output), "relay: posted HTTP 200\n");

    let raw = capture.finish();
    let sent = header_of(&raw, "x-webhook-signature").expect("a signature header");
    assert_eq!(
        sent,
        expected_signature("gate-signing-key", body_of(&raw)),
        "the signature must cover the bytes actually sent"
    );
}

#[test]
fn a_gateway_that_answers_401_is_named_rather_than_read_as_a_downed_gateway() {
    // "No response" would send the operator to restart a healthy gateway
    // instead of rotating the key.
    let sandbox = Sandbox::new("hermes-401");
    let auth = sandbox.write_auth("{\"hermes_secret\":\"gate-signing-key\"}\n");
    let capture = Capture::start(&sandbox, "hermes-401", Some("401"));

    let mut command = sandbox.bare();
    command
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_HERMES_URL", capture.url());
    sandbox.stub_notifier(&mut command);
    let output = run(command
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));
    capture.finish();

    assert_eq!(stdout(&output), "relay: post FAILED HTTP 401\n");
}

#[test]
fn an_async_hermes_with_a_real_key_stays_silent_even_when_the_post_fails() {
    // The alert-path silence check cannot see this: its auth carries no
    // hermes key, so that run returns before any outcome exists.
    let sandbox = Sandbox::new("hermes-async-silent");
    let auth = sandbox.write_auth("{\"hermes_secret\":\"gate-signing-key\"}\n");
    let mut command = sandbox.bare();
    command
        .env("RELAY_IDLE_SECS", "99999")
        .env("RELAY_AUTH_FILE", &auth)
        .env("RELAY_HERMES_URL", "http://127.0.0.1:1")
        .env("RELAY_MOSHI_URL", "http://127.0.0.1:1");
    sandbox.stub_notifier(&mut command);
    let output = run(command.args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        stdout(&output).is_empty(),
        "an async delivery printed an outcome; async legs must be silent: {output:?}"
    );
}
