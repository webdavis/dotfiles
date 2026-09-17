use super::*;
use pns_domain::CertificatePin;

fn pin(last: &str) -> CertificatePin {
    CertificatePin::parse(&format!("sha256:{}{last}", "0".repeat(62))).expect("a well formed pin")
}

#[test]
fn the_report_names_both_fingerprints_the_address_and_the_enrollment_command() {
    let text = body(&Mismatch {
        address: "192.0.2.1".to_string(),
        expected: pin("01"),
        presented: pin("02"),
    });
    assert!(text.contains("192.0.2.1"), "{text}");
    assert!(text.contains(&pin("01").to_string()), "{text}");
    assert!(text.contains(&pin("02").to_string()), "{text}");
    assert!(text.contains("pns lights enroll"), "{text}");
}
