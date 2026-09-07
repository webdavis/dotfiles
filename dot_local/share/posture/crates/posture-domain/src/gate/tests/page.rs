use super::*;
use Detector::*;

#[test]
fn c1_new_admin_user_pages() {
    assert_eq!(route(finding(NewAdminUser)), page());
}

#[test]
fn c2_added_filevault_off_pages() {
    assert_eq!(route(finding(FilevaultOff)), page());
}

#[test]
fn c3a_added_agent_exposure_pages() {
    assert_eq!(route(finding(AgentExposureChanged)), page());
}

#[test]
fn c3b_secret_file_changes_page_in_every_direction() {
    for action in [Action::Added, Action::Removed, Action::Other] {
        assert_eq!(
            route(GateFinding {
                action,
                ..finding(AgentSecretfileChanged)
            }),
            page()
        );
    }
}

#[test]
fn added_remote_access_and_setuid_binaries_page() {
    for detector in [RemoteAccessSharingState, SuidBinUnexpected] {
        assert_eq!(route(finding(detector)), page());
    }
}

#[test]
fn remote_authentication_file_events_page() {
    for target in [
        "/x/authorized_keys",
        "/x/authorized_keys2",
        "authorized_keys",
    ] {
        assert_eq!(route(file(FileCategory::Ssh, target)), page());
    }
    assert_eq!(
        route(file(FileCategory::SshdConfig, "/etc/ssh/sshd_config")),
        page()
    );
}

#[test]
fn extension_arms_honor_untrusted_promotion() {
    for detector in [KernelExtensionsNew, SystemExtensionsNew] {
        assert_eq!(
            signed(finding(detector), "UNSIGNED", true),
            GateOutcome::Page {
                signing: Some("UNSIGNED"),
                triage: None
            }
        );
    }
}

#[test]
fn c4b_reused_label_pages() {
    let mut row = finding(PersistenceLaunchd);
    row.columns.launchd = LaunchdIdentity {
        label: "com.good",
        path: "/x/good.plist",
        program: "/x/evil",
    };
    assert_eq!(
        gate(row, evidence(row), |identity| identity.program == "/x/good"),
        page()
    );
}

#[test]
fn c4c_unknown_user_agent_pages() {
    assert_eq!(route(finding(PersistenceLaunchd)), page());
}

#[test]
fn c4d_allowlisted_but_untrusted_program_pages() {
    assert_eq!(
        signed(finding(PersistenceLaunchd), "UNSIGNED", true),
        GateOutcome::Page {
            signing: Some("UNSIGNED"),
            triage: None
        }
    );
}

#[test]
fn launch_daemon_component_pages_before_allowlist() {
    let mut row = finding(PersistenceLaunchd);
    row.columns.launchd.path = "/Library/LaunchDaemons/agent.plist";
    assert_eq!(
        gate(row, evidence(row), |_| panic!(
            "daemon must not consult allowlist"
        )),
        page()
    );
}

#[test]
fn signing_text_is_attached_to_pages_trusted_or_untrusted() {
    for (text, untrusted) in [("signed: Apple", false), ("UNSIGNED", true)] {
        assert_eq!(
            signed(finding(SuidBinUnexpected), text, untrusted),
            GateOutcome::Page {
                signing: Some(text),
                triage: None
            }
        );
    }
}

#[test]
fn failed_enricher_nonempty_stdout_remains_on_a_page() {
    assert_eq!(
        signed(finding(SuidBinUnexpected), "partial signing fact", false),
        GateOutcome::Page {
            signing: Some("partial signing fact"),
            triage: None
        }
    );
}

#[test]
fn absent_or_empty_signing_does_not_lose_a_page() {
    assert_eq!(route(finding(SuidBinUnexpected)), page());
    assert_eq!(signed(finding(SuidBinUnexpected), "", false), page());
}

#[test]
fn untrusted_empty_stdout_still_promotes() {
    assert_eq!(signed(finding(KernelExtensionsNew), "", true), page());
}

#[test]
fn unavailable_severity_pages_the_fallback_detector() {
    let row = finding(HomebrewPackages);
    assert_eq!(
        gate(
            row,
            GateEvidence {
                severity: None,
                ..evidence(row)
            },
            |_| false
        ),
        page()
    );
}

#[test]
fn integrity_page_verdict_survives_absent_or_present_display_facts() {
    for category in [
        FileCategory::PipelineIntegrity,
        FileCategory::ManagedBin,
        FileCategory::LaunchAgents,
        FileCategory::LaunchDaemons,
        FileCategory::AllowlistFile,
    ] {
        let row = file(category, "/fixture/tracked");
        for triage in [
            None,
            Some(Triage {
                recorded: "abc",
                ondisk: "def",
                upgrade: "upgrade recorded",
            }),
        ] {
            assert_eq!(
                gate(
                    row,
                    GateEvidence {
                        triage,
                        ..evidence(row)
                    },
                    |_| false
                ),
                GateOutcome::Page {
                    signing: None,
                    triage
                }
            );
        }
    }
}

#[test]
fn display_facts_attach_only_to_integrity_pages() {
    let row = finding(NewAdminUser);
    let evidence = GateEvidence {
        triage: Some(Triage {
            recorded: "a",
            ondisk: "b",
            upgrade: "c",
        }),
        ..evidence(row)
    };
    assert_eq!(gate(row, evidence, |_| false), page());
}
