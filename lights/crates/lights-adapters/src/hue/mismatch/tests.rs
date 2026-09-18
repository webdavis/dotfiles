use super::*;

fn pin(last: &str) -> CertificatePin {
    CertificatePin::parse(&format!("sha256:{}{last}", "0".repeat(62))).unwrap()
}
fn mismatch() -> Mismatch {
    Mismatch {
        address: "192.0.2.1".to_string(),
        expected: pin("01"),
        presented: pin("02"),
    }
}

#[test]
fn a_mismatch_is_spoken_for_once_however_many_calls_refused() {
    let record = Record::default();
    for _ in 0..5 {
        record.keep(mismatch());
    }
    assert_eq!(record.unspoken(), Some(mismatch()));
    assert_eq!(record.unspoken(), None, "spoken for a second time");
}
#[test]
fn a_spoken_mismatch_is_still_readable_as_state() {
    let record = Record::default();
    record.keep(mismatch());
    assert!(record.unspoken().is_some());
    assert_eq!(record.refused(), Some(mismatch()));
}
#[test]
fn a_process_that_refused_nothing_has_nothing_to_say() {
    let record = Record::default();
    assert_eq!(record.unspoken(), None);
    assert_eq!(record.refused(), None);
}
#[test]
fn the_refusal_and_the_report_name_the_address_and_both_fingerprints() {
    for text in [
        refusal("192.0.2.1", pin("01"), pin("02")),
        report(&mismatch()),
    ] {
        assert!(text.contains("192.0.2.1"), "{text}");
        assert!(text.contains(&pin("01").to_string()), "{text}");
        assert!(text.contains(&pin("02").to_string()), "{text}");
    }
}
