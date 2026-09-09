use super::*;
use Detector::*;

#[test]
fn every_detector_has_one_page_digest_or_log_only_outcome() {
    use GateOutcome::LogOnly;
    for (detector, expected) in [
        (NewAdminUser, page()),
        (FileEventsRecent, LogOnly),
        (EsLaunchdWrites, LogOnly),
        (AgentAuthfileChanged, digest()),
        (AgentBinaryChanged, LogOnly),
        (AgentExposureChanged, page()),
        (AgentSecretfileChanged, page()),
        (ChromeExtensions, LogOnly),
        (FirefoxAddons, LogOnly),
        (HomebrewPackages, LogOnly),
        (InstalledApps, LogOnly),
        (SafariExtensions, LogOnly),
        (KernelExtensionsNew, LogOnly),
        (ListeningPortsNonLoopback, digest()),
        (PersistenceLaunchd, page()),
        (PersistenceLaunchdOverrides, LogOnly),
        (PersistenceStartupItemsCrontab, LogOnly),
        (RecentLogins, LogOnly),
        (SuidBinUnexpected, page()),
        (SystemExtensionsNew, digest()),
        (FilevaultOff, page()),
        (FilevaultState, LogOnly),
        (FirewallState, LogOnly),
        (GatekeeperState, LogOnly),
        (RemoteAccessSharingState, page()),
        (SipState, LogOnly),
    ] {
        assert_eq!(route(finding(detector)), expected, "{detector:?}");
    }
}
