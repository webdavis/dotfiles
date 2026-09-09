use super::*;
#[test]
fn first_exposures_combine_in_order_while_healthy_seed_and_recovery_are_silent() {
    let c = control("vault", "on", "");
    let observations = [observe(&c, ControlValue::Off)];
    let first = plan_poll(
        trio(["0", "0", "0"]),
        ControlsRead::Observed(&observations),
        None,
        &[],
        LuluProfile::Base,
    );
    let page = first.exposure_page.unwrap();
    assert_eq!(page.title, "🔴 **CRITICAL** · 4");
    let positions = [
        "Firewall is OFF",
        "Gatekeeper is OFF",
        "Screen lock is OFF",
        "`vault`",
    ]
    .map(|x| page.body.find(x).unwrap());
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    let healthy = [observe(&c, ControlValue::On)];
    let seeded = plan_poll(
        trio(["1", "1", "1"]),
        ControlsRead::Observed(&healthy),
        None,
        &[],
        LuluProfile::Base,
    );
    assert!(seeded.exposure_page.is_none());
    assert!(seeded.baseline.is_some());
    assert!(
        plan_poll(
            trio(["1", "1", "1"]),
            ControlsRead::Observed(&healthy),
            Some(previous(["0", "0", "0"], &[])),
            &[],
            LuluProfile::Base
        )
        .exposure_page
        .is_none()
    );
}
#[test]
fn steady_deviation_stays_silent_but_each_transition_to_off_pages() {
    for values in [["0", "1", "1"], ["1", "0", "1"], ["1", "1", "0"]] {
        let plan = plan_poll(
            trio(values),
            ControlsRead::Observed(&[]),
            Some(previous(["1", "1", "1"], &[])),
            &[],
            LuluProfile::Base,
        );
        assert!(plan.exposure_page.unwrap().body.contains("turned OFF"));
        assert!(
            plan_poll(
                trio(values),
                ControlsRead::Observed(&[]),
                Some(previous(values, &[])),
                &[],
                LuluProfile::Base
            )
            .exposure_page
            .is_none()
        );
    }
    assert!(
        plan_poll(
            trio(["1", "1", "1"]),
            ControlsRead::Observed(&[]),
            Some(previous(["2", "1", "1"], &[])),
            &[],
            LuluProfile::Base
        )
        .exposure_page
        .is_none()
    );
    let c = control("vault", "on", "");
    let readings = [observe(&c, ControlValue::Off)];
    for (before, pages) in [("on", true), ("off", false)] {
        let prior = [ControlPrior {
            id: "vault",
            value: before,
            expect: "on",
            target: "",
        }];
        assert_eq!(
            plan_poll(
                trio(["1", "1", "1"]),
                ControlsRead::Observed(&readings),
                Some(previous(["1", "1", "1"], &prior)),
                &[],
                LuluProfile::Base
            )
            .exposure_page
            .is_some(),
            pages
        );
    }
}
