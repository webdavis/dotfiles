use super::*;
use crate::hue::fixture_server::{CERTIFICATE, FIXTURE_ID, Server};
use crate::hue::pinned_tls::certificate_digest;

/// Long enough that no test here measures the machine's load.
const PATIENT: Duration = Duration::from_secs(10);

#[test]
fn an_enrollment_reads_the_presented_certificate_and_the_hosts_own_answer() {
    let server = Server::start(&format!(
        "{{\"bridgeid\":\"{FIXTURE_ID}\",\"modelid\":\"BSB003\"}}"
    ));

    let enrollment = enroll(&server.address(), PATIENT).expect("an enrollment");
    let _ = server.finish();
    assert_eq!(enrollment.common_name, FIXTURE_ID);
    assert_eq!(enrollment.reported_id.as_deref(), Some(FIXTURE_ID));
    assert_eq!(enrollment.model.as_deref(), Some("BSB003"));
    assert_eq!(
        enrollment.pin,
        CertificatePin::from_digest(certificate_digest(CERTIFICATE))
    );
    assert!(enrollment.identities_agree());
}

#[test]
fn a_host_whose_reported_id_is_not_its_certificates_name_does_not_agree_with_itself() {
    let server = Server::start("{\"bridgeid\":\"0000000000000000\",\"modelid\":\"BSB003\"}");

    let enrollment = enroll(&server.address(), PATIENT).expect("an enrollment");
    let _ = server.finish();
    assert_eq!(enrollment.common_name, FIXTURE_ID);
    assert!(!enrollment.identities_agree());
}

#[test]
fn a_host_that_answers_no_json_still_yields_a_fingerprint_and_no_reported_id() {
    let server = Server::start("not json");

    let enrollment = enroll(&server.address(), PATIENT).expect("an enrollment");
    let _ = server.finish();
    assert_eq!(enrollment.reported_id, None);
    assert!(!enrollment.identities_agree());
}

#[test]
fn a_host_nothing_answers_for_is_a_refusal_naming_the_address() {
    let refusal = enroll("127.0.0.1:9", Duration::from_millis(500)).expect_err("a refusal");
    assert!(refusal.contains("127.0.0.1:9"), "{refusal}");
}
