use super::*;
use Detector::*;

#[test]
fn unsafe_protections_new_admins_and_setuid_binaries_are_critical() {
    for detector in [FirewallState, GatekeeperState, SipState, FilevaultOff] {
        assert_eq!(
            severity(detector, Action::Added, ProtectionState::Off),
            Severity::Critical
        );
    }
    for detector in [NewAdminUser, SuidBinUnexpected] {
        for action in [Action::Added, Action::Removed, Action::Other] {
            assert_eq!(
                severity(detector, action, ProtectionState::Other),
                Severity::Critical
            );
        }
    }
    assert_eq!(
        severity(FilevaultOff, Action::Added, ProtectionState::Other),
        Severity::Critical
    );
}

#[test]
fn other_security_policy_rows_are_notice_never_info() {
    for detector in [
        FilevaultOff,
        FilevaultState,
        FirewallState,
        GatekeeperState,
        RemoteAccessSharingState,
        SipState,
    ] {
        for action in [Action::Removed, Action::Other] {
            for protection in [ProtectionState::Off, ProtectionState::Other] {
                assert_eq!(
                    severity(detector, action, protection),
                    Severity::Notice,
                    "{detector:?}"
                );
            }
        }
        if detector != FilevaultOff {
            assert_eq!(
                severity(detector, Action::Added, ProtectionState::Other),
                Severity::Notice
            );
        }
    }
}

#[test]
fn persistence_extensions_watched_files_and_endpoint_writes_are_notice() {
    for detector in [
        PersistenceLaunchd,
        PersistenceLaunchdOverrides,
        PersistenceStartupItemsCrontab,
        KernelExtensionsNew,
        SystemExtensionsNew,
        FileEventsRecent,
        EsLaunchdWrites,
    ] {
        assert_eq!(
            severity(detector, Action::Added, ProtectionState::Other),
            Severity::Notice
        );
    }
}

#[test]
fn software_listeners_logins_and_agent_queries_are_info() {
    for detector in [
        AgentAuthfileChanged,
        AgentBinaryChanged,
        AgentExposureChanged,
        AgentSecretfileChanged,
        ChromeExtensions,
        FirefoxAddons,
        HomebrewPackages,
        InstalledApps,
        SafariExtensions,
        ListeningPortsNonLoopback,
        RecentLogins,
    ] {
        assert_eq!(
            severity(detector, Action::Added, ProtectionState::Off),
            Severity::Info
        );
    }
}

#[test]
fn a_critical_finding_is_held_on_the_posture_route_until_priority_can_deliver() {
    // `priority` is where a page belongs and answers 401 to the key pns signs
    // with, so this pins the hold: flipping the arm has to come with the route.
    assert_eq!(severity_route(Some(Severity::Critical)), Some("posture"));
}

#[test]
fn every_tier_below_critical_belongs_on_the_posture_route() {
    for tier in [Severity::Notice, Severity::Info] {
        assert_eq!(severity_route(Some(tier)), Some("posture"), "{tier:?}");
    }
}

#[test]
fn a_submission_carrying_no_tier_names_no_route_of_its_own() {
    assert_eq!(severity_route(None), None);
}
