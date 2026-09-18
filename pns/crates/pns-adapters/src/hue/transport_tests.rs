use super::{Bridge, UreqBridge};
use crate::hue::pinned_tls::certificate_digest;
use pns_domain::CertificatePin;
use std::time::{Duration, Instant};

/// The pin the fixture's own certificate hashes to, DERIVED rather than
/// written down: a literal here would have to be re-measured by hand the day
/// the fixture pair is regenerated, and a stale one would read as a pinning
/// bug.
fn fixture_pin() -> CertificatePin {
    CertificatePin::from_digest(certificate_digest(include_bytes!(
        "transport_tests/cert.der"
    )))
}

/// A pin that is one bit away from the fixture's, which is the shape of a
/// changed certificate rather than of a typo.
fn flipped_pin() -> CertificatePin {
    let mut digest = certificate_digest(include_bytes!("transport_tests/cert.der"));
    digest[0] ^= 1;
    CertificatePin::from_digest(digest)
}

/// The bridge under test, pinned to whatever the caller means to pin it to.
fn bridge(base: String, pin: CertificatePin, deadline: Duration) -> UreqBridge {
    UreqBridge {
        base,
        key: "private-fixture-key".to_string(),
        deadline,
        pin,
    }
}

use crate::hue::fixture_server::{Reply, Server};

/// The deadline every test that is NOT about the deadline hands the bridge.
///
/// Long enough that it never expires, because those tests assert what came back
/// over the wire, not how fast. They used to share the deadline test's 150 ms,
/// which is a TLS handshake's budget on an idle machine and not on one running
/// 700 other tests: the handshake would lose, the bridge would report nothing,
/// and the assertion would read as a transport bug.
const PATIENT: Duration = Duration::from_secs(10);

/// The deadline the one deadline test hands the bridge, and the thing that test
/// measures. It has to cover a TLS handshake with room to spare, because a
/// caller that gives up mid-handshake never delivers the request the fixture is
/// asserted to have received; 150 ms did not cover one on a loaded machine.
// A SECOND RATHER THAN 400 ms. The deadline has to outlast the client
// reaching the fixture at all, and under a full parallel run a process spawn
// plus a loopback connect can miss 400 ms on work that is proceeding normally.
// When it did, the client gave up BEFORE the server ever accepted and the run
// failed on "the request reached the silent bridge", which is a lost race
// rather than a broken deadline.
const IMPATIENT: Duration = Duration::from_secs(1);

/// The ceiling on that measurement. A bridge that honours its deadline returns
/// at `IMPATIENT`; one that ignores it holds the connection until the fixture
/// runs out of patience, which is ten seconds away, so anything in between
/// separates the two even when the machine is busy.
// Raised with IMPATIENT and still FAR under the server's own PATIENT wait,
// which is the whole measurement: a client that ignored its deadline would sit
// here for ten seconds, not five.
const IMPATIENT_CEILING: Duration = Duration::from_secs(5);

#[test]
fn the_bridge_reads_through_its_self_signed_certificate_and_sends_the_key() {
    let server = Server::start(Reply::Body);
    let bridge = bridge(server.url(), fixture_pin(), PATIENT);

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
    let bridge = bridge(server.url(), fixture_pin(), PATIENT);

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
    let bridge = bridge(redirect.url(), fixture_pin(), PATIENT);

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
    let bridge = bridge(server.url(), fixture_pin(), IMPATIENT);
    let started = Instant::now();
    assert!(bridge.get("room").is_none());
    let elapsed = started.elapsed();
    assert!(
        server.finish().is_ok(),
        "the request reached the silent bridge"
    );
    assert!(
        elapsed >= IMPATIENT - Duration::from_millis(25),
        "returned before the deadline: {elapsed:?}"
    );
    assert!(
        elapsed < IMPATIENT_CEILING,
        "ignored the deadline: {elapsed:?}"
    );
}

#[test]
fn a_bridge_pinned_to_another_certificate_reads_nothing_and_never_sends_the_request() {
    let server = Server::start(Reply::Body);
    let bridge = bridge(server.url(), flipped_pin(), PATIENT);

    let response = bridge.get("room");
    assert_eq!(response, None, "read through an unpinned certificate");
    assert!(
        server.finish().is_err(),
        "the request reached a bridge whose certificate was refused"
    );
}

#[test]
fn a_put_through_a_mismatched_pin_writes_nothing_and_does_not_fail_its_caller() {
    let server = Server::start(Reply::Body);
    let bridge = bridge(server.url(), flipped_pin(), PATIENT);

    bridge.put("light/fixture", "{\"on\":{\"on\":false}}");
    assert!(
        server.finish().is_err(),
        "the write reached a bridge whose certificate was refused"
    );
}

#[test]
fn a_refused_handshake_records_the_mismatch_with_both_fingerprints() {
    let server = Server::start(Reply::Body);
    let bridge = bridge(server.url(), flipped_pin(), PATIENT);

    assert_eq!(bridge.get("room"), None);
    let _ = server.finish();
    let recorded = crate::hue::mismatch::refused_mismatch().expect("a recorded mismatch");
    assert_eq!(recorded.presented, fixture_pin());
}
