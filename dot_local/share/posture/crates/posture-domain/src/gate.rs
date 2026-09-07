use crate::{Action, Detector, Severity};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LaunchdIdentity<'a> {
    pub label: &'a str,
    pub path: &'a str,
    pub program: &'a str,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileCategory {
    Ssh,
    SshdConfig,
    PipelineIntegrity,
    ManagedBin,
    LaunchAgents,
    LaunchDaemons,
    AllowlistFile,
    Sudoers,
    #[default]
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GateColumns<'a> {
    pub launchd: LaunchdIdentity<'a>,
    pub file_category: FileCategory,
    pub target_path: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateFinding<'a> {
    pub detector: Detector,
    pub action: Action,
    pub enrichment_path: &'a str,
    pub columns: GateColumns<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signing<'a> {
    pub untrusted: bool,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triage<'a> {
    pub recorded: &'a str,
    pub ondisk: &'a str,
    pub upgrade: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityVerdict {
    Page,
    LogOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateEvidence<'a> {
    pub severity: Option<Severity>,
    pub signing: Option<Signing<'a>>,
    pub integrity: IntegrityVerdict,
    pub triage: Option<Triage<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome<'a> {
    Page {
        signing: Option<&'a str>,
        triage: Option<Triage<'a>>,
    },
    Digest {
        signing: Option<&'a str>,
    },
    LogOnly,
}

pub fn gate<'a>(
    finding: GateFinding<'a>,
    evidence: GateEvidence<'a>,
    allowlisted: impl FnOnce(LaunchdIdentity<'a>) -> bool,
) -> GateOutcome<'a> {
    use Detector::*;
    // Resolve the missing tier before the explicit detector overrides, as Bash does.
    let mut severity = evidence.severity.unwrap_or(Severity::Critical);
    let signing = evidence
        .signing
        .filter(|_| !finding.enrichment_path.is_empty() && severity != Severity::Info);
    if signing.is_some_and(|signing| signing.untrusted) && severity == Severity::Notice {
        severity = Severity::Critical;
    }
    // A failed enricher can still provide text; only its untrusted verdict promotes.
    let signing = signing
        .map(|signing| signing.text)
        .filter(|text| !text.is_empty());
    let page = GateOutcome::Page {
        signing,
        triage: None,
    };
    let digest = GateOutcome::Digest { signing };
    let critical = if severity == Severity::Critical {
        page
    } else {
        GateOutcome::LogOnly
    };
    match finding.detector {
        FirewallState
        | GatekeeperState
        | SipState
        | PersistenceStartupItemsCrontab
        | EsLaunchdWrites
        | AgentBinaryChanged => GateOutcome::LogOnly,
        KernelExtensionsNew => critical,
        AgentAuthfileChanged => digest,
        SystemExtensionsNew => {
            if severity == Severity::Critical {
                page
            } else {
                digest
            }
        }
        ListeningPortsNonLoopback => {
            if finding.action == Action::Added {
                digest
            } else {
                GateOutcome::LogOnly
            }
        }
        AgentSecretfileChanged => page,
        AgentExposureChanged | RemoteAccessSharingState => {
            if finding.action == Action::Added {
                page
            } else {
                GateOutcome::LogOnly
            }
        }
        SuidBinUnexpected => {
            if finding.action == Action::Added {
                critical
            } else {
                GateOutcome::LogOnly
            }
        }
        PersistenceLaunchd => {
            let identity = finding.columns.launchd;
            if finding.action != Action::Added || identity.path.starts_with("/System/Library/") {
                GateOutcome::LogOnly
            } else if identity.path.contains("/LaunchDaemons/") || !allowlisted(identity) {
                page
            } else {
                critical
            }
        }
        FileEventsRecent => match finding.columns.file_category {
            FileCategory::Ssh => match finding.columns.target_path.rsplit('/').next() {
                Some("authorized_keys" | "authorized_keys2") => page,
                _ => digest,
            },
            FileCategory::SshdConfig => page,
            FileCategory::PipelineIntegrity
            | FileCategory::ManagedBin
            | FileCategory::LaunchAgents
            | FileCategory::LaunchDaemons
            | FileCategory::AllowlistFile => match evidence.integrity {
                IntegrityVerdict::Page => GateOutcome::Page {
                    signing,
                    triage: evidence.triage,
                },
                IntegrityVerdict::LogOnly => GateOutcome::LogOnly,
            },
            FileCategory::Sudoers => digest,
            FileCategory::Other => GateOutcome::LogOnly,
        },
        _ => critical,
    }
}

#[cfg(test)]
mod tests;
