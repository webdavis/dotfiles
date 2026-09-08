use super::fixtures::{elsewhere, names, three_selection, watching};
use crate::{DecisionRequest, EnvironmentSnapshot, Overrides, SilencePolicy};

#[test]
fn a_class_exception_preserves_only_the_selected_banner_and_phone_under_each_silence() {
    for (muted, focus_active) in [(true, false), (false, true), (true, true)] {
        for (idle, expected) in [(2, "macos-banner"), (9_000, "mobile")] {
            let decide = |silence_policy, skip_phone, local_only, remote_only, visible| {
                crate::decide(
                    &EnvironmentSnapshot {
                        idle: Some(idle),
                        view: Some(if visible {
                            watching("wW:p1")
                        } else {
                            elsewhere("wW:p1")
                        }),
                        ..Default::default()
                    },
                    &three_selection(),
                    &Overrides {
                        muted,
                        focus_active,
                        skip_phone,
                        ..Default::default()
                    },
                    DecisionRequest {
                        local_only,
                        remote_only,
                        pane: "wW:p1",
                        now_secs: Some(1_000_000),
                        long_running: true,
                        mobile_watch_card: false,
                        silence_policy,
                    },
                )
            };
            let selected = decide(
                SilencePolicy::BypassBannerAndPhone,
                false,
                false,
                false,
                false,
            );
            assert_eq!(
                names(&selected),
                [expected, "hermes"],
                "{muted}/{focus_active}/{idle}"
            );
            assert!(!selected.plan.pulse, "class does not unmute lights");
            assert_eq!(
                names(&decide(SilencePolicy::Respect, false, false, false, false)),
                ["hermes"]
            );
            assert_eq!(
                names(&decide(
                    SilencePolicy::BypassBannerAndPhone,
                    false,
                    false,
                    true,
                    false
                )),
                ["hermes"]
            );
            assert!(
                names(&decide(
                    SilencePolicy::BypassBannerAndPhone,
                    false,
                    true,
                    true,
                    false
                ))
                .is_empty()
            );
            if idle > 120 {
                assert_eq!(
                    names(&decide(
                        SilencePolicy::BypassBannerAndPhone,
                        true,
                        false,
                        false,
                        false
                    )),
                    ["hermes"],
                    "skip phone still wins"
                );
            } else {
                assert_eq!(
                    names(&decide(
                        SilencePolicy::BypassBannerAndPhone,
                        false,
                        false,
                        false,
                        true
                    )),
                    ["hermes"],
                    "visible pane stays undecorated"
                );
            }
        }
    }
}
