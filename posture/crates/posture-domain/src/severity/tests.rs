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
