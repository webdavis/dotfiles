use super::*;
use crate::{ProtectionState, severity};

mod digest;
mod fields;
mod log_only;
mod outcomes;
mod page;

fn finding(detector: Detector) -> GateFinding<'static> {
    GateFinding {
        detector,
        action: Action::Added,
        enrichment_path: "",
        columns: GateColumns::default(),
    }
}

fn evidence(finding: GateFinding<'_>) -> GateEvidence<'static> {
    GateEvidence {
        severity: Some(severity(
            finding.detector,
            finding.action,
            ProtectionState::Other,
        )),
        signing: None,
        integrity: IntegrityVerdict::Page,
        triage: None,
    }
}

fn route(finding: GateFinding<'_>) -> GateOutcome<'_> {
    gate(finding, evidence(finding), |_| false)
}

fn page() -> GateOutcome<'static> {
    GateOutcome::Page {
        signing: None,
        triage: None,
    }
}

fn digest() -> GateOutcome<'static> {
    GateOutcome::Digest { signing: None }
}

fn file(category: FileCategory, target: &str) -> GateFinding<'_> {
    GateFinding {
        columns: GateColumns {
            file_category: category,
            target_path: target,
            ..Default::default()
        },
        ..finding(Detector::FileEventsRecent)
    }
}

fn signed<'a>(mut finding: GateFinding<'a>, text: &'a str, untrusted: bool) -> GateOutcome<'a> {
    finding.enrichment_path = "/fixture/program";
    let mut evidence = evidence(finding);
    evidence.signing = Some(Signing { text, untrusted });
    gate(finding, evidence, |_| true)
}
