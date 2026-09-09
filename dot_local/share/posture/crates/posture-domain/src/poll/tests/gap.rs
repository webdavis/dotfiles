use super::*;
#[test]
fn missing_controls_and_first_firewall_off_produce_two_independent_pages() {
    let refused = validate_controls(ControlsInput::Missing("/missing-controls.json")).unwrap_err();
    let plan = plan_poll(
        trio(["0", "1", "1"]),
        ControlsRead::Refused(&refused),
        None,
        &[],
        LuluProfile::Base,
    );
    assert_eq!(plan.gap_members, ["controls_file"]);
    assert_eq!(
        plan.gap_page.unwrap().body,
        include_str!("missing_controls_gap.txt")
    );
    let exposure = plan.exposure_page.unwrap();
    assert_eq!(exposure.title, "🔴 **CRITICAL**");
    assert_eq!(exposure.body, include_str!("firewall_first.txt"));
    assert_eq!(exposure.sound, "Sosumi");
    assert_eq!(exposure.severity, Severity::Critical);
    assert_eq!(plan.baseline.unwrap().trio.values(), [0, 1, 1]);
}
#[test]
fn each_gap_member_pages_on_arrival_and_rearms_after_its_own_recovery() {
    let a = control("a", "on", "");
    let b = control("b", "on", "");
    let readings = [
        ControlObservation {
            control: &a,
            reading: ControlReading::Indeterminate,
        },
        ControlObservation {
            control: &b,
            reading: ControlReading::Indeterminate,
        },
    ];
    let next = |covered: &[&str], values: &[ControlObservation<'_>]| {
        plan_poll(
            trio(["1", "1", "1"]),
            ControlsRead::Observed(values),
            None,
            covered,
            LuluProfile::Base,
        )
    };
    assert_eq!(next(&[], &readings).gap_members, ["a", "b"]);
    assert!(next(&["a"], &readings).gap_page.is_some());
    assert!(next(&["a", "b"], &readings).gap_page.is_none());
    let recovered = [observe(&a, ControlValue::On), readings[1]];
    assert_eq!(next(&["a", "b"], &recovered).gap_members, ["b"]);
    assert!(next(&["b"], &readings).gap_page.is_some());
    let healthy = [observe(&a, ControlValue::On), observe(&b, ControlValue::On)];
    assert!(next(&["a", "b"], &healthy).gap_members.is_empty());
}
#[test]
fn an_unreadable_trio_without_a_trusted_prior_stops_after_the_gap() {
    let c = control("vault", "on", "");
    let readings = [observe(&c, ControlValue::Off)];
    let plan = plan_poll(
        TrioReading {
            values: ["1", "1", "1"],
            exit: 1,
        },
        ControlsRead::Observed(&readings),
        None,
        &[],
        LuluProfile::Base,
    );
    assert_eq!(plan.gap_members, ["posture_query"]);
    assert!(plan.gap_page.is_some());
    assert!(plan.exposure_page.is_none());
    assert!(plan.baseline.is_none());
}
#[test]
fn one_gapped_control_cannot_blind_clean_members_and_only_its_prior_is_kept() {
    let a = control("a", "on", "");
    let b = control("b", "on", "");
    let priors = [
        ControlPrior {
            id: "a",
            value: "on",
            expect: "on",
            target: "",
        },
        ControlPrior {
            id: "b",
            value: "on",
            expect: "on",
            target: "",
        },
    ];
    let readings = [
        ControlObservation {
            control: &a,
            reading: ControlReading::Indeterminate,
        },
        observe(&b, ControlValue::Off),
    ];
    let plan = plan_poll(
        trio(["0", "1", "1"]),
        ControlsRead::Observed(&readings),
        Some(previous(["1", "1", "1"], &priors)),
        &[],
        LuluProfile::Base,
    );
    assert_eq!(plan.gap_members, ["a"]);
    assert!(plan.gap_page.is_some());
    assert_eq!(plan.exposure_page.unwrap().title, "🔴 **CRITICAL** · 2");
    let next = plan.baseline.unwrap();
    assert_eq!(
        next.controls
            .iter()
            .map(|x| (x.id.as_str(), x.value))
            .collect::<Vec<_>>(),
        [("a", ControlValue::On), ("b", ControlValue::Off)]
    );
}
#[test]
fn refused_controls_preserve_prior_fields_and_profile_uncertainty_blinds_only_rule_readers() {
    let old = [ControlPrior {
        id: "a",
        value: "off",
        expect: "on",
        target: "",
    }];
    let refused = validate_controls(ControlsInput::Malformed).unwrap_err();
    let preserved = plan_poll(
        trio(["2", "1", "1"]),
        ControlsRead::Refused(&refused),
        Some(previous(["1", "1", "1"], &old)),
        &[],
        LuluProfile::Base,
    )
    .baseline
    .unwrap();
    assert!(preserved.preserve_prior_fields);
    assert_eq!(preserved.trio.values(), [2, 1, 1]);
    let rule = control("rule", "present", "/tool");
    let vault = control("vault", "on", "");
    let readings = [
        observe(&rule, ControlValue::Present),
        observe(&vault, ControlValue::Off),
    ];
    for profile in [LuluProfile::Active, LuluProfile::Unconfirmed] {
        let plan = plan_poll(
            trio(["1", "1", "1"]),
            ControlsRead::Observed(&readings),
            None,
            &[],
            profile,
        );
        assert_eq!(plan.gap_members, ["rule"]);
        assert!(plan.gap_page.unwrap().body.contains("LuLu"));
        assert!(plan.exposure_page.unwrap().body.contains("vault"));
        assert_eq!(plan.baseline.unwrap().controls.len(), 1);
    }
}
