use super::*;
use pns_domain::CertificatePin;

fn enrollment(common_name: &str, reported: Option<&str>) -> pns_adapters::Enrollment {
    pns_adapters::Enrollment {
        common_name: common_name.to_string(),
        pin: CertificatePin::parse(&format!("sha256:{}1", "0".repeat(63)))
            .expect("a well formed pin"),
        reported_id: reported.map(str::to_string),
        model: Some("BSB003".to_string()),
    }
}

#[test]
fn a_certificate_and_a_reported_id_that_agree_pass_with_the_cost_of_skipping_stated() {
    let warning = verdict(&enrollment("ABC123", Some("ABC123")), None)
        .expect("an enrollment that may be saved")
        .expect("the warning that the identity was not checked out of band");
    assert!(warning.contains("--bridge-id"), "{warning}");
    assert!(warning.contains("impostor"), "{warning}");
}

#[test]
fn a_certificate_and_a_reported_id_that_disagree_are_refused_naming_both() {
    let refusal =
        verdict(&enrollment("ABC123", Some("DEF456")), None).expect_err("a refusal to enroll");
    assert!(refusal.contains("ABC123"), "{refusal}");
    assert!(refusal.contains("DEF456"), "{refusal}");
}

#[test]
fn a_host_that_reports_no_id_at_all_is_refused_rather_than_trusted() {
    assert!(verdict(&enrollment("ABC123", None), None).is_err());
}

#[test]
fn a_stated_id_that_matches_the_certificate_passes_with_no_warning_at_all() {
    assert_eq!(
        verdict(&enrollment("ABC123", Some("ABC123")), Some("abc123")),
        Ok(None)
    );
}

#[test]
fn a_stated_id_the_certificate_does_not_carry_is_refused_even_when_the_host_agrees() {
    let refusal = verdict(&enrollment("ABC123", Some("ABC123")), Some("OTHER"))
        .expect_err("a refusal to enroll");
    assert!(refusal.contains("OTHER"), "{refusal}");
    assert!(refusal.contains("label"), "{refusal}");
}

#[test]
fn bridge_id_or_an_explicit_opt_out_is_required_and_anything_else_refuses() {
    assert_eq!(stated_id(&[]), None);
    assert_eq!(stated_id(&["--bogus".to_string()]), None);
    assert_eq!(
        stated_id(&["--bridge-id".to_string(), "ABC123".to_string()]),
        Some(Some("ABC123".to_string()))
    );
    assert_eq!(stated_id(&["--allow-unverified".to_string()]), Some(None));
}

#[test]
fn the_readings_name_every_field_the_operator_compares() {
    let lines = readings(&enrollment("ABC123", Some("ABC123"))).join("\n");
    assert!(lines.contains("ABC123"), "{lines}");
    assert!(lines.contains("BSB003"), "{lines}");
    assert!(lines.contains("sha256:"), "{lines}");
}
