//! The two or three lines that let the operator decide without opening a laptop.
//!
//! Every detector shows only what the decision turns on. The rule the whole file
//! serves: a field an attacker chooses is rendered as data, never as markdown.

use super::PageFinding;
use crate::Detector;
use crate::sanitize;

/// What a missing column renders as.
const UNKNOWN: &str = "?";

/// The decision lines for one finding, in order.
pub(super) fn fields(finding: &PageFinding<'_>) -> Vec<String> {
    let columns = &finding.columns;
    let signing = signing_line(finding);
    match finding.detector() {
        Some(Detector::PersistenceLaunchd) => {
            let mut lines = vec![
                labelled("What", columns.label),
                labelled("Program", columns.program),
            ];
            lines.extend(signing);
            lines
        }
        Some(Detector::PersistenceStartupItemsCrontab) => {
            let mut lines = vec![
                labelled("What", columns.name),
                labelled("Command", columns.command),
            ];
            lines.extend(signing);
            lines
        }
        Some(Detector::SuidBinUnexpected) => {
            // OWNER AFTER THE SIGNING VERDICT, deliberately: the verdict is what
            // decides, and the owner is the detail that follows from it.
            let mut lines = vec![labelled("Path", columns.path)];
            lines.extend(signing);
            lines.push(labelled("Owner", columns.username));
            lines
        }
        Some(Detector::NewAdminUser) => vec![
            labelled("User", columns.username),
            labelled("UID", columns.uid),
        ],
        Some(Detector::AgentExposureChanged) => vec![
            labelled("Process", columns.name),
            labelled("Address", columns.address),
            labelled("Port", columns.port),
        ],
        // THE BASENAME AND NOTHING ELSE. These two detectors reach the page as a
        // change to a credential or a secret, and the page fans out to Discord,
        // so neither the path nor any content hash may leave the machine. No
        // hash field is rendered for them at all.
        Some(Detector::AgentAuthfileChanged | Detector::AgentSecretfileChanged) => {
            let basename = sanitize::basename(columns.path.unwrap_or_default());
            vec![format!("- **File:** {}", sanitize::code(basename))]
        }
        Some(Detector::RemoteAccessSharingState) => vec![labelled("Service", columns.service)],
        Some(Detector::SystemExtensionsNew) => {
            let mut lines = vec![
                labelled("Name", columns.identifier),
                labelled("Team", columns.team),
            ];
            lines.extend(signing);
            lines
        }
        Some(Detector::KernelExtensionsNew) => {
            let mut lines = vec![
                labelled("Name", columns.name),
                labelled("Path", columns.path),
            ];
            lines.extend(signing);
            lines
        }
        Some(Detector::FileEventsRecent) => watched_file_fields(finding),
        Some(Detector::EsLaunchdWrites) => {
            let mut lines = vec![
                labelled("Process", columns.path),
                labelled("Wrote", columns.filename.or(columns.dest_filename)),
            ];
            lines.extend(signing);
            lines
        }
        _ if super::header::protection_name(finding.detector()).is_some() => {
            vec!["- **State:** **OFF**".to_string()]
        }
        _ => {
            let mut lines = signing;
            lines.push(format!(
                "- **What:** {}",
                sanitize::code(key_identifier(finding))
            ));
            lines
        }
    }
}

/// The watched file's own lines, plus the triage facts when the router gathered
/// them.
fn watched_file_fields(finding: &PageFinding<'_>) -> Vec<String> {
    let mut lines = vec![
        labelled("File", finding.columns.target_path),
        format!(
            "- **Action:** {}",
            // The action's exception, stated where it is taken: squashed only.
            sanitize::squash(finding.columns.action.or(finding.act).unwrap_or_default())
        ),
    ];
    if let Some(triage) = &finding.triage {
        // WHICH BYTES DISAGREE, and whether a recorded upgrade plausibly
        // explains it. The upgrade sentence quotes package names chosen by
        // whoever published them, so it goes through the chokepoint too.
        lines.push(format!(
            "- **Recorded:** {} · **On disk:** {}",
            sanitize::code(triage.recorded),
            sanitize::code(triage.ondisk)
        ));
        lines.push(format!(
            "- **Upgrade record:** {}",
            sanitize::code(triage.upgrade)
        ));
    }
    lines
}

/// The signing verdict line, when the enricher reached one.
///
/// An untrusted verdict is bolded behind a warning glyph, because it is the one
/// field that turns a curiosity into a decision.
fn signing_line(finding: &PageFinding<'_>) -> Vec<String> {
    let Some(signing) = &finding.signing else {
        return Vec::new();
    };
    let text = sanitize::signing(signing.text);
    if signing.untrusted {
        vec![format!("- **Signing:** ⚠️ **{text}**")]
    } else {
        vec![format!("- **Signing:** {text}")]
    }
}

/// The best single name for a finding no detector claims.
///
/// The order is most specific first, so a row carrying several of these names
/// is identified by the one that says most.
fn key_identifier<'a>(finding: &PageFinding<'a>) -> &'a str {
    let columns = &finding.columns;
    columns
        .label
        .or(columns.identifier)
        .or(columns.name)
        .or(columns.target_path)
        .or(columns.path)
        .or(columns.username)
        .unwrap_or(UNKNOWN)
}

/// One `- **Label:** value` line, with the value code-wrapped.
fn labelled(label: &str, value: Option<&str>) -> String {
    format!(
        "- **{label}:** {}",
        sanitize::code(value.unwrap_or(UNKNOWN))
    )
}
