use super::*;
use lights_adapters::Enrollment;
use lights_domain::CertificatePin;

fn enrollment(common_name: &str, reported: Option<&str>) -> Enrollment {
    Enrollment {
        common_name: common_name.to_string(),
        pin: CertificatePin::parse(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        reported_id: reported.map(str::to_string),
        model: None,
    }
}

#[test]
fn an_id_read_off_the_device_that_names_the_certificate_passes_without_a_warning() {
    assert_eq!(
        verdict(&enrollment("B0B0", Some("B0B0")), Some("b0b0")),
        Ok(None)
    );
}
#[test]
fn an_id_read_off_the_device_that_names_another_bridge_refuses() {
    let refusal = verdict(&enrollment("B0B0", Some("B0B0")), Some("dead")).unwrap_err();
    assert!(refusal.contains("B0B0"), "{refusal}");
    assert!(refusal.contains("dead"), "{refusal}");
}
#[test]
fn a_host_that_agrees_with_itself_passes_and_says_what_the_missing_check_costs() {
    let warning = verdict(&enrollment("B0B0", Some("b0b0")), None)
        .unwrap()
        .expect("a warning");
    assert!(warning.contains("--bridge-id"), "{warning}");
    assert!(warning.contains("impostor"), "{warning}");
}
#[test]
fn a_host_that_disagrees_with_itself_refuses_even_without_an_id() {
    for reported in [None, Some("dead")] {
        assert!(
            verdict(&enrollment("B0B0", reported), None).is_err(),
            "{reported:?}"
        );
    }
}
