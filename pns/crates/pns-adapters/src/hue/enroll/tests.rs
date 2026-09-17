use super::*;
use crate::hue::pinned_tls::certificate_digest;

use crate::hue::fixture_server::{Reply, Server};

/// The fixture's certificate names this, and the scripted `/api/config`
/// answers say the same thing when the two are meant to agree.
const FIXTURE_NAME: &str = "pns-private-fixture.invalid";

const PATIENT: Duration = Duration::from_secs(10);

#[test]
fn an_enrollment_reads_the_presented_certificate_and_the_hosts_own_answer() {
    let server = Server::start(Reply::Json(format!(
        "{{\"bridgeid\":\"{FIXTURE_NAME}\",\"modelid\":\"BSB003\"}}"
    )));
    let address = server.url().replace("https://", "");

    let enrollment = enroll(&address, PATIENT).expect("an enrollment over the fixture");
    let _ = server.finish();
    assert_eq!(enrollment.common_name, FIXTURE_NAME);
    assert_eq!(enrollment.reported_id.as_deref(), Some(FIXTURE_NAME));
    assert_eq!(enrollment.model.as_deref(), Some("BSB003"));
    assert_eq!(
        enrollment.pin,
        CertificatePin::from_digest(certificate_digest(include_bytes!(
            "../transport_tests/cert.der"
        )))
    );
    assert!(enrollment.identities_agree());
}

#[test]
fn a_host_whose_reported_id_is_not_the_certificates_name_does_not_agree_with_itself() {
    let server = Server::start(Reply::Json(
        "{\"bridgeid\":\"C42996FFFECB6DCC\",\"modelid\":\"BSB003\"}".to_string(),
    ));
    let address = server.url().replace("https://", "");

    let enrollment = enroll(&address, PATIENT).expect("an enrollment over the fixture");
    let _ = server.finish();
    assert_eq!(enrollment.common_name, FIXTURE_NAME);
    assert!(!enrollment.identities_agree());
}

#[test]
fn a_host_that_answers_no_json_still_yields_a_fingerprint_and_no_reported_id() {
    let server = Server::start(Reply::Body);
    let address = server.url().replace("https://", "");

    let enrollment = enroll(&address, PATIENT).expect("an enrollment over the fixture");
    let _ = server.finish();
    assert_eq!(enrollment.reported_id, None);
    assert!(!enrollment.identities_agree());
}

#[test]
fn a_bridge_nothing_answers_for_is_a_refusal_naming_the_address() {
    let refusal = enroll("127.0.0.1:9", Duration::from_millis(500)).expect_err("a refusal");
    assert!(refusal.contains("127.0.0.1:9"), "{refusal}");
}
