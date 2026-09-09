use super::*;
#[test]
fn baseline_requires_exact_mode_one_object_and_every_trio_scalar_domain() {
    for fw in ["0", "1", "2"] {
        for gk in ["0", "1"] {
            for sl in ["0", "1"] {
                assert_eq!(
                    previous([fw, gk, sl], &[]).trio.values(),
                    [
                        fw.parse().unwrap(),
                        gk.parse().unwrap(),
                        sl.parse().unwrap()
                    ]
                );
            }
        }
    }
    for mode in [None, Some(0o577), Some(0o601), Some(0o640)] {
        assert!(trusted_poll_baseline(mode, true, ["1", "1", "1"], &[]).is_none());
    }
    assert!(trusted_poll_baseline(Some(0o600), false, ["1", "1", "1"], &[]).is_none());
    for values in [
        ["-1", "1", "1"],
        ["3", "1", "1"],
        ["1", "2", "1"],
        ["1", "1", "2"],
        ["01", "1", "1"],
        ["1\n", "1", "1"],
    ] {
        assert!(trusted_poll_baseline(Some(0o600), true, values, &[]).is_none());
    }
}
#[test]
fn control_priors_are_rearmed_independently_when_expect_target_or_domain_changes() {
    let c = control("rule", "present", "/new");
    let observations = [observe(&c, ControlValue::Absent)];
    for prior in [
        ControlPrior {
            id: "rule",
            value: "absent",
            expect: "present",
            target: "/old",
        },
        ControlPrior {
            id: "rule",
            value: "absent",
            expect: "absent",
            target: "/new",
        },
        ControlPrior {
            id: "rule",
            value: "on",
            expect: "present",
            target: "/new",
        },
    ] {
        let priors = [prior];
        let plan = plan_poll(
            trio(["1", "1", "1"]),
            ControlsRead::Observed(&observations),
            Some(previous(["1", "1", "1"], &priors)),
            &[],
            LuluProfile::Base,
        );
        assert!(
            plan.exposure_page
                .unwrap()
                .body
                .contains("at first observation")
        );
        assert_eq!(plan.baseline.unwrap().controls[0].target, "/new");
    }
    let priors = [ControlPrior {
        id: "rule",
        value: "absent",
        expect: "present",
        target: "/new",
    }];
    assert!(
        plan_poll(
            trio(["1", "1", "1"]),
            ControlsRead::Observed(&observations),
            Some(previous(["1", "1", "1"], &priors)),
            &[],
            LuluProfile::Base
        )
        .exposure_page
        .is_none()
    );
}
