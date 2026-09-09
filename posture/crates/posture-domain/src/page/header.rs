//! The one line that says what happened, in words rather than query names.
//!
//! The operator reads this on a phone, mid-something-else. A header that says
//! `agent_secretfile_changed` costs a translation step at the worst moment, so
//! every admitted detector has a plain-English name and anything unmapped falls
//! back to its own name with the underscores opened out.

use super::PageFinding;
use crate::sanitize;
use crate::{Detector, Severity};

/// Our own osquery LaunchAgent plists, which arrive under the launch-agent
/// category rather than the pipeline one and would otherwise read as an
/// ordinary startup-folder change.
const OUR_AGENT_PREFIX: &str = "com.webdavis.osquery-";

/// What a macOS protection query is called in plain English, if it is one.
///
/// A PROTECTION READS DIFFERENTLY FROM EVERYTHING ELSE: the header states that
/// it is off rather than that something appeared, because the absence is the
/// finding. Turned OFF is reserved for the critical severity; anything else
/// changed.
pub(super) fn protection_name(detector: Option<Detector>) -> Option<&'static str> {
    match detector? {
        Detector::FirewallState => Some("Firewall"),
        Detector::GatekeeperState => Some("Gatekeeper"),
        Detector::SipState => Some("System Integrity Protection"),
        Detector::FilevaultState | Detector::FilevaultOff => Some("FileVault"),
        _ => None,
    }
}

/// The finding's header line, without its bold markers.
pub(super) fn header(finding: &PageFinding<'_>) -> String {
    let detector = finding.detector();
    if let Some(protection) = protection_name(detector) {
        let state = if finding.severity == Severity::Critical {
            "turned OFF"
        } else {
            "changed"
        };
        return format!("{protection} {state}");
    }
    match detector {
        Some(Detector::PersistenceLaunchd) => "New startup item".to_string(),
        Some(Detector::PersistenceLaunchdOverrides) => "Startup override changed".to_string(),
        Some(Detector::PersistenceStartupItemsCrontab) => "New startup/cron entry".to_string(),
        Some(Detector::SuidBinUnexpected) => "New setuid root binary".to_string(),
        Some(Detector::NewAdminUser) => "New administrator account".to_string(),
        Some(Detector::AgentExposureChanged) => "Agent port exposed off-loopback".to_string(),
        Some(Detector::AgentAuthfileChanged) => "Agent credential changed".to_string(),
        Some(Detector::AgentSecretfileChanged) => "Agent secret file changed".to_string(),
        Some(Detector::RemoteAccessSharingState) => "Remote-access service enabled".to_string(),
        Some(Detector::KernelExtensionsNew) => "New kernel extension".to_string(),
        Some(Detector::SystemExtensionsNew) => "New system extension".to_string(),
        Some(Detector::ListeningPortsNonLoopback) => "New network listener".to_string(),
        Some(Detector::RecentLogins) => "Login".to_string(),
        Some(Detector::InstalledApps) => "New app".to_string(),
        Some(Detector::HomebrewPackages) => "New Homebrew package".to_string(),
        Some(Detector::ChromeExtensions | Detector::FirefoxAddons | Detector::SafariExtensions) => {
            "New browser extension".to_string()
        }
        Some(Detector::FileEventsRecent) => watched_file_header(finding).to_string(),
        Some(Detector::EsLaunchdWrites) => "Startup item written by a process".to_string(),
        _ => finding.query.replace('_', " "),
    }
}

/// What a watched file's change is called.
///
/// BASENAME BEFORE CATEGORY, and the order is the point. Our own osquery
/// LaunchAgent plists sit in the launch-agent category beside every other
/// startup file, so reading the category first would call a change to the
/// alerting pipeline an ordinary startup-folder change, which is the one
/// finding that must not blend in.
pub(super) fn watched_file_header(finding: &PageFinding<'_>) -> &'static str {
    if is_our_security_tooling(finding) {
        return "Security tooling changed";
    }
    match finding.columns.category.unwrap_or_default() {
        "ssh" | "authorized_keys" => "SSH key file changed",
        "sudoers" => "sudoers changed",
        "sshd_config" => "sshd_config changed",
        "pipeline_integrity" => "Security tooling changed",
        "allowlist_file" => "Allowlist changed",
        "launch_agents" | "launch_daemons" => "Startup folder changed",
        _ => "Watched file changed",
    }
}

/// Is this a change to the alerting pipeline's own files?
///
/// Shared with the next step, which asks the same question to decide whether to
/// offer a hash comparison or a permissions review.
pub(super) fn is_our_security_tooling(finding: &PageFinding<'_>) -> bool {
    let basename = sanitize::basename(finding.columns.target_path.unwrap_or_default());
    basename.starts_with(OUR_AGENT_PREFIX) && basename.ends_with(".plist")
}
