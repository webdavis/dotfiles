//! The native plugins: the branch no stub channel can reach.
//!
//! Leaving `PNS_CHANNELS_DIR` unset is the only condition under which the
//! compiled-in plugins win, so these stub `terminal-notifier` on PATH and
//! point the channel URLs at the crate's one-shot capture binary instead.
//! std only on both sides, so there is no interpreter cold start to
//! diagnose when one fails.

mod support;

use hmac::{Hmac, KeyInit, Mac};
use support::{
    Capture, KEYS_DISAGREE, RouterStub, Sandbox, plugin_command, router_table, run, run_expecting,
    stderr, stdout, write_script,
};

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
        &decoy.join("banner.sh"),
        &format!("cat >\"{}/decoy.event\"", sandbox.display()),
    );

    let mut command = plugin_command(&sandbox);
    // At the desk, because the banner is a desk surface now: an idle of
    // 99999 is the operator being away, and away raises no banner at all.
    command.env("PNS_SCREEN_IDLE", "0");
    sandbox.stub_notifier(&mut command);
    run(command
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
        .args(["--scope", "local_only"]));

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
    sandbox.write_config(
        "[plugins.phone]\nenabled = true\ntype = \"moshi\"\ndevice_token = \"tok-integration\"\n",
    );
    let capture = Capture::builder(&sandbox, "phone").start();

    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_SCREEN_IDLE", "99999")
        .env("PNS_MOSHI_URL", capture.url());
    sandbox.stub_notifier(&mut command);
    let output = run(command.args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]));

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
    sandbox.write_config(
        "[plugins.phone]\nenabled = true\ntype = \"moshi\"\ndevice_token = \"tok-integration\"\n",
    );
    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_SCREEN_IDLE", "99999")
        .env("PNS_MOSHI_URL", "http://127.0.0.1:1");
    sandbox.stub_notifier(&mut command);
    let output = run(command.args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]));
    assert!(
        stderr(&output).is_empty(),
        "a failed post said something; the failure path must be silent: {output:?}"
    );
}

#[test]
fn sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent() {
    let sandbox = Sandbox::new("native-hermes");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"gate-signing-key\" }\n",
    );
    let capture = Capture::builder(&sandbox, "hermes").start();

    let mut command = plugin_command(&sandbox);
    command.env("PNS_HERMES_URL", capture.url());
    sandbox.stub_notifier(&mut command);
    let output = run(command
        .args([
            "send",
            "--producer",
            "weekly",
            "--state",
            "done",
            "--detail",
            "ran",
        ])
        .args(["--scope", "remote_only"]));

    assert_eq!(stdout(&output), "pns: posted HTTP 200\n");

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
    // "No response" would send the operator to restart a healthy gateway.
    let sandbox = Sandbox::new("hermes-401");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"gate-signing-key\" }\n",
    );
    let capture = Capture::builder(&sandbox, "hermes-401").status(401).start();

    let mut command = plugin_command(&sandbox);
    command.env("PNS_HERMES_URL", capture.url());
    sandbox.stub_notifier(&mut command);
    command.args([
        "send",
        "--producer",
        "weekly",
        "--state",
        "done",
        "--detail",
        "ran",
        "--scope",
        "remote_only",
    ]);
    let output = run_expecting(1, &mut command);
    capture.finish();
    assert_eq!(stdout(&output), "pns: post FAILED HTTP 401\n");
}

#[test]
fn an_async_hermes_with_a_real_key_stays_silent_even_when_the_post_fails() {
    // The alert-path silence check cannot see this: its config carries no
    // hermes key, so that run returns before any outcome exists.
    let sandbox = Sandbox::new("hermes-async-silent");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"gate-signing-key\" }\n",
    );
    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_SCREEN_IDLE", "99999")
        .env("PNS_HERMES_URL", "http://127.0.0.1:1");
    sandbox.stub_notifier(&mut command);
    command.args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]);
    let output = run_expecting(1, &mut command);
    assert!(
        stdout(&output).is_empty(),
        "an async delivery printed an outcome; async legs must be silent: {output:?}"
    );
}

/// The route the config named, ON THE WIRE.
///
/// THE ONE ASSERTION NO STUB CHANNEL CAN MAKE. `stale_alert_route` reading a
/// name and `channel_url` swapping a path segment are each pinned by unit
/// tests; what nothing pinned is the ASSIGNMENT of the one onto the other, and
/// dropping the route from the event passed the entire suite. Every other home
/// test sets `PNS_CHANNELS_DIR`, which leaves the native hermes channel
/// computing a URL nothing sends, and `PNS_HERMES_URL` outranks the route, so
/// an endpoint override cannot observe it either.
///
/// SO THE GATEWAY IS PROXIED RATHER THAN MOVED. The engine posts to its own
/// compile-time default, port 8644, which this test does not own and must not
/// bind; `HTTP_PROXY` is what sends that connection to the capture server
/// instead. ureq takes its proxy from the environment by default and opens the
/// connection with CONNECT, so the capture answers the tunnel and reads the
/// request inside it, forwarding nothing. `NO_PROXY` keeps the router stub
/// direct, and since a bypass matches on HOST alone the stub is addressed as
/// `localhost` while the gateway stays `127.0.0.1`.
#[test]
fn the_stale_alert_posts_to_the_hermes_route_the_config_named() {
    let sandbox = Sandbox::new("stale-alert-route");
    let router = RouterStub::start(KEYS_DISAGREE);
    // TWO REQUESTS, because the report posts its own test send before it
    // reads the router: the capture has to stay up past that one to see the
    // alert at all.
    let capture = Capture::builder(&sandbox, "stale").requests(2).start();
    sandbox.write_config(&format!(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\n\
         keys = {{ pns-events = \"gate-signing-key\", priority = \"priority-signing-key\" }}\n\
         {}alert_route = \"priority\"\n",
        router_table(&router.localhost_url())
    ));

    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_SCREEN_IDLE", "99999")
        .env("HTTP_PROXY", capture.url())
        .env("http_proxy", capture.url())
        .env("NO_PROXY", "localhost");
    sandbox.stub_notifier(&mut command);
    // No moshi-hook to spawn: the pairing check would otherwise reach the real
    // binary on the developer's own machine and the moshi API behind it.
    command.env("PNS_MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    // THE EXIT CODE IS NOT ASSERTED: it belongs to the whole report, and a
    // config naming one plugin makes it exit non-zero on grounds that have
    // nothing to do with the route this pins.
    command.arg("doctor").output().expect("the engine runs");

    let raw = capture.finish();
    assert!(
        raw.lines()
            .any(|line| line == "POST /webhooks/priority HTTP/1.1"),
        "the alert did not carry the configured route: {raw}"
    );
    // AND THE GATEWAY IS UNMOVED. The config names a ROUTE, never a URL, so
    // the host and port must still be the compiled-in default: a swap that
    // took the base with it would be a different defect passing this test.
    assert_eq!(
        header_of(&raw, "host").as_deref(),
        Some("127.0.0.1:8644"),
        "the route swap moved the gateway too: {raw}"
    );
}

/// THE ROUTE NAMES ARE THE OPERATOR'S, ON THE WIRE.
///
/// THE ONE ASSERTION NO UNIT TEST CAN MAKE. That `[routes]` parses, that a
/// delivery class takes the route its own table names and that a key table
/// grants a route its key are each pinned on their own; what nothing pinned is
/// the ASSIGNMENT of all three onto one POST, and every compiled route name
/// this change removed used to be what carried it. So the gateway is PROXIED rather than moved,
/// exactly as the stale alert's own route test does it and for its reason:
/// `PNS_HERMES_URL` outranks the route, so an endpoint override cannot observe
/// one.
///
/// NOT ONE SHIPPED NAME IN THE CONFIG. Neither route below is a name this
/// repository ships a default for, and neither is granted a key under one, so
/// a roster compiled back in would refuse this file at load or sign the post
/// with a key it was never granted.
#[test]
fn a_health_event_takes_the_urgent_route_the_config_invented_and_signs_it_with_that_routes_key() {
    let sandbox = Sandbox::new("invented-routes");
    let capture = Capture::builder(&sandbox, "invented-routes").start();
    sandbox.write_config(
        "[routes]\ndefault = \"logbook\"\nurgent = \"sirens\"\n\
         [delivery_class.health]\nroute = \"sirens\"\n[plugins.log]\nenabled = true\ntype = \"hermes\"\n\
         keys = { logbook = \"logbook-key\", sirens = \"sirens-key\" }\n",
    );

    let mut command = plugin_command(&sandbox);
    command
        .env("HTTP_PROXY", capture.url())
        .env("http_proxy", capture.url());
    sandbox.stub_notifier(&mut command);
    run(command
        .args([
            "send",
            "--producer",
            "upgrades",
            "--state",
            "failed",
            "--detail",
            "ran",
        ])
        .args(["--delivery-class", "health"])
        .args(["--scope", "remote_only"]));

    let raw = capture.finish();
    assert_eq!(
        raw.lines().next().unwrap_or_default(),
        "POST /webhooks/sirens HTTP/1.1",
        "the health event did not take the configured urgent route: {raw}"
    );
    // AND THE GATEWAY IS UNMOVED: the config names ROUTES, never URLs.
    assert_eq!(
        header_of(&raw, "host").as_deref(),
        Some("127.0.0.1:8644"),
        "the route swap moved the gateway too: {raw}"
    );
    // AND SIGNED WITH THAT ROUTE'S OWN KEY, which is the half a path cannot
    // show: the default route's key would verify nowhere on this route.
    assert_eq!(
        header_of(&raw, "x-webhook-signature"),
        Some(expected_signature("sirens-key", body_of(&raw))),
        "the post was signed with a key the sirens route was never granted: {raw}"
    );
}

/// AND A SESSION EVENT TAKES THE CONFIGURED DEFAULT ROUTE, on the same wire.
///
/// THE OTHER HALF OF THE PAIR ABOVE, and the mutant it pins is different: a
/// default route resolved from a compiled name rather than the table would put
/// every unrouted event on a route this gateway does not serve, which is the
/// whole durable log gone at 404.
#[test]
fn an_unrouted_event_takes_the_default_route_the_config_invented() {
    let sandbox = Sandbox::new("invented-default-route");
    let capture = Capture::builder(&sandbox, "invented-default").start();
    sandbox.write_config(
        "[routes]\ndefault = \"logbook\"\n\
         [plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { logbook = \"logbook-key\" }\n",
    );

    let mut command = plugin_command(&sandbox);
    command
        .env("HTTP_PROXY", capture.url())
        .env("http_proxy", capture.url());
    sandbox.stub_notifier(&mut command);
    run(command
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
        .args(["--scope", "remote_only"]));

    let raw = capture.finish();
    assert_eq!(
        raw.lines().next().unwrap_or_default(),
        "POST /webhooks/logbook HTTP/1.1",
        "the event did not take the configured default route: {raw}"
    );
}
