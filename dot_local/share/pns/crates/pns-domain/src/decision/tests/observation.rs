use super::fixtures::{elsewhere, names, three_selection, watching};
use crate::{DecisionRequest, EnvironmentSnapshot, Overrides, SilencePolicy};

fn request() -> DecisionRequest<'static> {
    DecisionRequest {
        observation: true,
        local_only: false,
        remote_only: false,
        pane: "t1:p1",
        now_secs: Some(1_000_000),
        long_running: true,
        mobile_watch_card: true,
        silence_policy: SilencePolicy::Respect,
    }
}

#[test]
fn observations_keep_only_banner_and_hermes_on_every_surface_despite_phone_overrides() {
    for (idle, phone_age) in [(2, 9_000), (9_000, 2), (9_000, 9_000)] {
        for visible in [false, true] {
            for (force_phone, skip_phone) in [(false, false), (true, false), (true, true)] {
                let decision = crate::decide(
                    &EnvironmentSnapshot {
                        idle: Some(idle),
                        view: Some(if visible {
                            watching("t1:p1")
                        } else {
                            elsewhere("t1:p1")
                        }),
                        ..Default::default()
                    },
                    &three_selection(),
                    &Overrides {
                        phone_input_age: Some(phone_age),
                        force_phone,
                        skip_phone,
                        ..Default::default()
                    },
                    request(),
                );
                assert_eq!(names(&decision), ["macos-banner", "hermes"]);
                assert!(decision.plan.banner);
                assert!(!decision.plan.phone_card);
                assert!(!decision.plan.pulse);
            }
        }
    }
}

#[test]
fn observation_scope_and_disabled_plugins_still_narrow_delivery() {
    for (local_only, remote_only, expected) in [
        (false, false, vec!["macos-banner", "hermes"]),
        (true, false, vec!["macos-banner"]),
        (false, true, vec!["hermes"]),
        (true, true, vec![]),
    ] {
        let selected = crate::decide(
            &EnvironmentSnapshot::default(),
            &three_selection(),
            &Overrides::default(),
            DecisionRequest {
                local_only,
                remote_only,
                ..request()
            },
        );
        assert_eq!(names(&selected), expected);
    }
    for enabled in ["hermes", "macos-banner", "mobile"] {
        let selection = crate::registry::roster()
            .enabled(&std::collections::BTreeMap::from([
                ("hermes".into(), enabled == "hermes"),
                ("macos-banner".into(), enabled == "macos-banner"),
                ("mobile".into(), enabled == "mobile"),
            ]))
            .unwrap();
        let selected = crate::decide(
            &EnvironmentSnapshot::default(),
            &selection,
            &Overrides::default(),
            request(),
        );
        let expected = if enabled == "mobile" {
            vec![]
        } else {
            vec![enabled]
        };
        assert_eq!(names(&selected), expected);
    }
}

#[test]
fn observations_respect_mute_and_focus_without_restoring_phone_or_pulse() {
    for (muted, focus_active) in [(true, false), (false, true), (true, true)] {
        for silence_policy in [SilencePolicy::Respect, SilencePolicy::BypassBannerAndPhone] {
            let selected = crate::decide(
                &EnvironmentSnapshot::default(),
                &three_selection(),
                &Overrides {
                    muted,
                    focus_active,
                    force_phone: true,
                    ..Default::default()
                },
                DecisionRequest {
                    silence_policy,
                    ..request()
                },
            );
            assert_eq!(
                names(&selected),
                if silence_policy == SilencePolicy::Respect {
                    vec!["hermes"]
                } else {
                    vec!["macos-banner", "hermes"]
                }
            );
            assert!(!selected.plan.phone_card);
            assert!(!selected.plan.pulse);
        }
    }
}
