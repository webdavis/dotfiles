use super::{Bridge, UreqBridge};
use std::time::{Duration, Instant};

#[path = "transport_tests/server.rs"]
mod server;
use server::{Reply, Server};

#[test]
fn the_bridge_reads_through_its_self_signed_certificate_and_sends_the_key() {
    let server = Server::start(Reply::Body);
    let bridge = UreqBridge {
        base: server.url(),
        key: "private-fixture-key".to_string(),
        deadline: Duration::from_millis(150),
    };

    let response = bridge.get("room");
    let captured = server.finish();
    assert_eq!(response.as_deref(), Some("{\"data\":[]}"));
    let request = captured.expect("a complete bridge request");
    assert!(request.starts_with("GET /room HTTP/1.1\r\n"), "{request}");
    assert!(
        request
            .to_lowercase()
            .contains("hue-application-key: private-fixture-key\r\n"),
        "{request}"
    );
}

#[test]
fn the_bridge_puts_the_exact_body_through_its_self_signed_certificate() {
    let server = Server::start(Reply::Body);
    let bridge = UreqBridge {
        base: server.url(),
        key: "private-fixture-key".to_string(),
        deadline: Duration::from_millis(150),
    };

    bridge.put("light/fixture", "{\"on\":{\"on\":false}}");
    let request = server.finish().expect("a complete bridge write");
    assert!(
        request.starts_with("PUT /light/fixture HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(
        request.ends_with("\r\n\r\n{\"on\":{\"on\":false}}"),
        "{request}"
    );
    assert!(
        request
            .to_lowercase()
            .contains("hue-application-key: private-fixture-key\r\n"),
        "{request}"
    );
}

#[test]
fn a_bridge_redirect_body_is_returned_without_following_the_location() {
    let destination = Server::start(Reply::Body);
    let redirect = Server::start(Reply::Redirect(destination.url()));
    let bridge = UreqBridge {
        base: redirect.url(),
        key: "private-fixture-key".to_string(),
        deadline: Duration::from_millis(150),
    };

    let response = bridge.get("room");
    let original = redirect.finish();
    let forwarded = destination.finish();
    assert_eq!(response.as_deref(), Some(""), "the original response body");
    assert!(original.is_ok(), "the original bridge was contacted");
    assert!(
        forwarded.is_err(),
        "the redirect destination received a request"
    );
}

#[test]
fn a_bridge_that_does_not_answer_spends_only_the_callers_deadline() {
    let server = Server::start(Reply::Wait);
    let bridge = UreqBridge {
        base: server.url(),
        key: "private-fixture-key".to_string(),
        deadline: Duration::from_millis(150),
    };
    let started = Instant::now();
    assert!(bridge.get("room").is_none());
    let elapsed = started.elapsed();
    assert!(
        server.finish().is_ok(),
        "the request reached the silent bridge"
    );
    assert!(
        elapsed >= Duration::from_millis(125),
        "returned before the deadline: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(275),
        "ignored the deadline: {elapsed:?}"
    );
}
