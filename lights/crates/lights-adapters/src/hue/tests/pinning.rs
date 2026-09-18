use super::super::fixture_server::{CERTIFICATE, Server};
use super::super::pinned_tls::certificate_digest;
use crate::hue::HueLightController;
use lights_application::LightController;
use lights_domain::{CertificatePin, RoomName};

/// The pin the fixture's own certificate hashes to, DERIVED rather than written
/// down: a literal would have to be re-measured by hand the day the fixture
/// pair is regenerated, and a stale one would read as a pinning bug.
fn fixture_pin() -> CertificatePin {
    CertificatePin::from_digest(certificate_digest(CERTIFICATE))
}

/// A pin one bit away from the fixture's, which is the shape of a changed
/// certificate rather than of a typo.
fn flipped_pin() -> CertificatePin {
    let mut digest = certificate_digest(CERTIFICATE);
    digest[0] ^= 1;
    CertificatePin::from_digest(digest)
}

fn controller(address: &str, pin: CertificatePin) -> HueLightController {
    let mut settings = crate::settings::parse(&format!(
        "[controller]\ntype='hue'\naddress='192.0.2.1'\nkey='test-secret'\ntimeout_secs=10\n\
certificate='{}'\n",
        fixture_pin()
    ))
    .unwrap()
    .controller;
    settings.address = address.to_string();
    settings.certificate = pin;
    HueLightController::new(&settings)
}

#[test]
fn a_read_through_the_pinned_certificate_reaches_the_bridge_and_sends_the_key() {
    let server = Server::start("{\"errors\":[],\"data\":[]}");
    let controller = controller(&server.address(), fixture_pin());

    let refusal = controller.room(&RoomName::new("Studio").unwrap());
    let request = server.finish().expect("a complete request");
    assert!(
        request.starts_with("GET /clip/v2/resource HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(
        request
            .to_lowercase()
            .contains("hue-application-key: test-secret\r\n"),
        "{request}"
    );
    // An empty bridge knows no rooms, which is the answer of a bridge that was
    // REACHED rather than refused.
    assert!(
        matches!(
            refusal,
            Err(lights_application::LightControlError::UnknownRoom { .. })
        ),
        "{refusal:?}"
    );
}

/// ONE TEST FOR THE WHOLE REFUSAL, because the record it reads is process
/// scoped and spoken for once: two tests refusing a handshake in one test
/// binary would race for the same first report.
#[test]
fn a_read_pinned_to_another_certificate_is_refused_before_any_request_is_sent() {
    let server = Server::start("{\"errors\":[],\"data\":[]}");
    let controller = controller(&server.address(), flipped_pin());

    let failure = controller.room(&RoomName::new("Studio").unwrap());
    assert!(
        server.finish().is_err(),
        "the request reached a bridge whose certificate was refused"
    );
    let detail = match failure {
        Err(lights_application::LightControlError::Refused { detail }) => detail,
        other => panic!("{other:?}"),
    };
    assert!(detail.contains(&fixture_pin().to_string()), "{detail}");
    assert!(detail.contains(&flipped_pin().to_string()), "{detail}");
    let recorded = crate::hue::mismatch::refused_mismatch().expect("a recorded mismatch");
    assert_eq!(recorded.presented, fixture_pin());
    assert_eq!(recorded.expected, flipped_pin());
}
