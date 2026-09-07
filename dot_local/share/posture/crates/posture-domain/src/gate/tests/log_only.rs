use super::*;
use Detector::*;

#[test]
fn safe_directions_remain_log_only() {
    for detector in [
        FilevaultOff,
        AgentExposureChanged,
        RemoteAccessSharingState,
        ListeningPortsNonLoopback,
        PersistenceLaunchd,
    ] {
        for action in [Action::Removed, Action::Other] {
            assert_eq!(
                route(GateFinding {
                    action,
                    ..finding(detector)
                }),
                GateOutcome::LogOnly
            );
        }
    }
}

#[test]
fn removed_setuid_is_log_only_even_with_untrusted_signing() {
    let row = GateFinding {
        action: Action::Removed,
        ..finding(SuidBinUnexpected)
    };
    assert_eq!(route(row), GateOutcome::LogOnly);
    assert_eq!(signed(row, "UNSIGNED", true), GateOutcome::LogOnly);
}

#[test]
fn sip_off_is_log_only_despite_critical_base_severity() {
    let row = finding(SipState);
    let observed = severity(SipState, Action::Added, ProtectionState::Off);
    assert_eq!(observed, Severity::Critical);
    assert_eq!(
        gate(
            row,
            GateEvidence {
                severity: Some(observed),
                ..evidence(row)
            },
            |_| false
        ),
        GateOutcome::LogOnly
    );
    assert_eq!(
        gate(
            row,
            GateEvidence {
                severity: None,
                ..evidence(row)
            },
            |_| false
        ),
        GateOutcome::LogOnly
    );
}

#[test]
fn explicit_log_only_arms_ignore_untrusted_promotion() {
    for detector in [
        FirewallState,
        GatekeeperState,
        SipState,
        PersistenceStartupItemsCrontab,
        EsLaunchdWrites,
        AgentBinaryChanged,
    ] {
        assert_eq!(
            signed(finding(detector), "UNSIGNED", true),
            GateOutcome::LogOnly
        );
    }
}

#[test]
fn c4a_full_allowlisted_tuple_is_suppressed() {
    assert_eq!(
        signed(finding(PersistenceLaunchd), "signed: Apple", false),
        GateOutcome::LogOnly
    );
}

#[test]
fn apple_prefix_is_log_only_before_allowlist_and_signing() {
    for path in [
        "/System/Library/LaunchAgents/x",
        "/System/Library/",
        "/System/Library/LaunchDaemons/x",
    ] {
        let mut row = finding(PersistenceLaunchd);
        row.columns.launchd.path = path;
        assert_eq!(
            gate(row, evidence(row), |_| panic!(
                "Apple item must not consult allowlist"
            )),
            GateOutcome::LogOnly
        );
        assert_eq!(signed(row, "UNSIGNED", true), GateOutcome::LogOnly);
    }
}

#[test]
fn path_prefix_and_component_neighbours_still_use_the_allowlist() {
    for path in [
        "/System/Library",
        "/System/Libraryevil/x",
        "/x/LaunchDaemons",
        "/x/LaunchDaemonsevil/x",
    ] {
        let mut row = finding(PersistenceLaunchd);
        row.columns.launchd.path = path;
        assert_eq!(gate(row, evidence(row), |_| true), GateOutcome::LogOnly);
        assert_eq!(gate(row, evidence(row), |_| false), page());
    }
}

#[test]
fn kernel_extension_without_promotion_remains_log_only() {
    assert_eq!(route(finding(KernelExtensionsNew)), GateOutcome::LogOnly);
    assert_eq!(
        signed(finding(KernelExtensionsNew), "partial signing fact", false),
        GateOutcome::LogOnly
    );
}

#[test]
fn absent_path_and_info_severity_do_not_consume_signing() {
    let row = finding(KernelExtensionsNew);
    let signing = Some(Signing {
        text: "UNSIGNED",
        untrusted: true,
    });
    assert_eq!(
        gate(
            row,
            GateEvidence {
                signing,
                ..evidence(row)
            },
            |_| false
        ),
        GateOutcome::LogOnly
    );
    let row = GateFinding {
        enrichment_path: "/fixture/program",
        ..finding(AgentSecretfileChanged)
    };
    assert_eq!(
        gate(
            row,
            GateEvidence {
                signing,
                ..evidence(row)
            },
            |_| false
        ),
        page()
    );
}

#[test]
fn integrity_silent_verdict_and_untracked_neighbours_stay_silent() {
    for category in [
        FileCategory::PipelineIntegrity,
        FileCategory::ManagedBin,
        FileCategory::LaunchAgents,
        FileCategory::LaunchDaemons,
        FileCategory::AllowlistFile,
    ] {
        let row = file(category, "/fixture/neighbour");
        assert_eq!(
            gate(
                row,
                GateEvidence {
                    integrity: IntegrityVerdict::LogOnly,
                    ..evidence(row)
                },
                |_| false
            ),
            GateOutcome::LogOnly
        );
    }
}

#[test]
fn unrecognized_file_categories_are_log_only() {
    assert_eq!(
        route(file(FileCategory::Other, "/fixture/file")),
        GateOutcome::LogOnly
    );
}

#[test]
fn healthy_fallback_detectors_stay_log_only() {
    for detector in [
        FilevaultState,
        ChromeExtensions,
        FirefoxAddons,
        HomebrewPackages,
        InstalledApps,
        SafariExtensions,
        RecentLogins,
        PersistenceLaunchdOverrides,
    ] {
        assert_eq!(route(finding(detector)), GateOutcome::LogOnly);
    }
}
