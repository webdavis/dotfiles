//! The one action the block ends on.
//!
//! Every block closes with a question the operator can answer from memory ("did
//! you do this?") and, where there is one, a command to run. The question comes
//! first because it is the cheap half: an operator who recognizes the change
//! stops reading there.

use super::PageFinding;
use crate::Detector;
use crate::sanitize;

/// The closing lines for one finding.
pub(super) fn next_step(finding: &PageFinding<'_>) -> Vec<String> {
    let path = finding.enrichment_path;
    if super::header::protection_name(finding.detector()).is_some() {
        return lines(&[
            "- Did you turn this off? If not, something else did - **investigate now**.",
            "- Re-enable it in System Settings.",
        ]);
    }
    match finding.detector() {
        Some(Detector::SystemExtensionsNew | Detector::KernelExtensionsNew) => lines(&[
            "- Did you install this? If not, **remove it** - an extension can intercept traffic or load at boot.",
            "- Manage at: System Settings → General → Login Items & Extensions",
        ]),
        Some(Detector::SuidBinUnexpected) => vec![
            "- Did you create this? If not, it lets a program run as **root** - a backdoor.".to_string(),
            command("Inspect", "codesign -dv --", path),
        ],
        Some(Detector::NewAdminUser) => lines(&[
            "- Did you create this account? If not, someone gained **admin access** - investigate now.",
            "- Review accounts: System Settings → Users & Groups",
        ]),
        Some(Detector::AgentExposureChanged) => lines(&[
            "- Did you expose this? If not, an agent API is reachable **off-box** - close it now.",
            "- Re-bind it to 127.0.0.1 or block the port at the firewall.",
        ]),
        Some(Detector::AgentAuthfileChanged) => lines(&[
            "- Did you rotate this? If not, an attacker may forge or mute alerts, or hijack remote access - **investigate now**.",
        ]),
        Some(Detector::AgentSecretfileChanged) => lines(&[
            "- Did you rotate this? If not, an attacker may have your alerting or remote-access secret - **investigate now**.",
        ]),
        Some(Detector::RemoteAccessSharingState) => lines(&[
            "- Did you enable this? If not, someone opened a remote-control path into this Mac - **disable it now**.",
            "- System Settings → General → Sharing",
        ]),
        Some(Detector::FileEventsRecent) => watched_file_next_step(finding),
        Some(Detector::PersistenceLaunchd | Detector::PersistenceStartupItemsCrontab) => vec![
            "- Did you set this up? If not, it **auto-runs at every login** - likely malware.".to_string(),
            command("Inspect", "cat --", path),
        ],
        Some(Detector::EsLaunchdWrites) => vec![
            "- Did you run this? If not, a process is **installing persistence** - investigate it and remove the file.".to_string(),
            command("Inspect the writer", "codesign -dv --", path),
        ],
        // Anything else offers the enrichment path itself, and only when there
        // is one: a bare "Review:" with nothing after it is noise.
        _ if !path.is_empty() => {
            vec![format!("- **Review:** {}", sanitize::code(path))]
        }
        _ => Vec::new(),
    }
}

/// The closing lines for a watched file, which split on whether the file is ours.
fn watched_file_next_step(finding: &PageFinding<'_>) -> Vec<String> {
    let path = finding.enrichment_path;
    let ours = finding.columns.category.unwrap_or_default() == "pipeline_integrity"
        || super::header::is_our_security_tooling(finding);
    if ours {
        return vec![
            "- Did you just apply your dotfiles? If not, your **security tooling was modified** - investigate now.".to_string(),
            command("Compare", "shasum -a 256 --", path),
        ];
    }
    vec![
        "- Did you change this? If not, someone altered who can log in or run as **root**."
            .to_string(),
        command("Review", "sudo cat --", path),
    ]
}

/// A `- **Label:** `command 'path'`` line.
///
/// THE PATH IS SHELL-QUOTED BEFORE IT IS CODE-WRAPPED, and both steps matter in
/// that order. Quoting makes the path a single argument if the operator pastes
/// the line; wrapping keeps the quotes visible rather than letting markdown eat
/// them.
fn command(label: &str, program: &str, path: &str) -> String {
    format!(
        "- **{label}:** {}",
        sanitize::code(&format!("{program} {}", sanitize::shell_quote(path)))
    )
}

fn lines(literals: &[&str]) -> Vec<String> {
    literals.iter().map(|line| (*line).to_string()).collect()
}
