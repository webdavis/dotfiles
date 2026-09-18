use super::*;

fn pin(last: &str) -> CertificatePin {
    CertificatePin::parse(&format!("sha256:{}{last}", "0".repeat(62))).expect("a well formed pin")
}

fn mismatch() -> Mismatch {
    Mismatch {
        address: "192.0.2.1".to_string(),
        expected: pin("01"),
        presented: pin("02"),
    }
}

#[test]
fn a_mismatch_carries_both_fingerprints_out_once_however_many_calls_refused() {
    let observer = Observer::default();
    for _ in 0..5 {
        observer.record(mismatch());
    }

    let spoken = observer.unreported().expect("the first mismatch");
    assert_eq!(spoken.expected, pin("01"));
    assert_eq!(spoken.presented, pin("02"));
    assert_eq!(spoken.address, "192.0.2.1");
    assert_eq!(observer.unreported(), None, "reported a second time");
}

#[test]
fn a_reported_mismatch_is_still_readable_as_state() {
    let observer = Observer::default();
    observer.record(mismatch());
    assert!(observer.unreported().is_some());
    assert_eq!(observer.refused(), Some(mismatch()));
}

#[test]
fn a_process_that_refused_nothing_has_nothing_to_report() {
    let observer = Observer::default();
    assert_eq!(observer.unreported(), None);
    assert_eq!(observer.refused(), None);
}

#[test]
fn the_refusal_names_the_address_and_both_fingerprints() {
    let text = refusal("192.0.2.1", pin("01"), pin("02"));
    assert!(text.contains("192.0.2.1"), "{text}");
    assert!(text.contains(&pin("01").to_string()), "{text}");
    assert!(text.contains(&pin("02").to_string()), "{text}");
}
