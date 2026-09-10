//! The judge: one batch of results-log rows in, at most one page out.
//!
//! THIS IS ORDERING, NOT POLICY. Every verdict here belongs to
//! `posture-domain`: `severity` ranks a finding, `gate` decides page or digest
//! or nothing, and `render_page` writes the body. What this owns is the
//! sequence, and the two collaborators the gate needs answers from, which both
//! reach outside the process.
//!
//! A DIGEST ROW IS DELIVERED THE MOMENT IT IS SPOOLED, which is why only a page
//! can fail this run. The transaction above only has to hold the checkpoint
//! against the page's delivery.
//!
//! WHAT IT REFUSES TO DO IS DECIDE WHY A ROW WAS QUIET. A row that gates to
//! `LogOnly` is not written anywhere: the shell dropped it the same way, and a
//! log of every uninteresting row is the file the rotation then rotates a real
//! log out of.

use super::{AllowlistText, DigestAppendFile, ResultsRow, rows};
use posture_application::{BatchPage, JudgeFindings, JudgedBatch};
use posture_domain::{
    Action, Allowlist, Detector, GateEvidence, GateFinding, GateOutcome, IntegrityVerdict,
    LaunchdIdentity, PageFinding, Severity, Signing, Triage, allowlist_verdict, gate, render_page,
    severity,
};

/// What the gate needs that is not on the row: whether a file's current bytes
/// are vouched for, and what a spawned inspection said about a binary.
///
/// TWO CLOSURES RATHER THAN TWO TRAITS, because each is asked exactly once per
/// row and neither has state of its own. The composition root supplies the real
/// ones; a test supplies answers.
pub struct Collaborators<'a> {
    /// Whether the pipeline manifest vouches for the file at this path as it
    /// stands right now, content mode and owner together.
    pub vouches: &'a mut dyn FnMut(&str) -> bool,
    /// The signing verdict for an enrichment path, when one was resolved.
    pub inspect: &'a mut dyn FnMut(&str) -> Option<OwnedSigning>,
    /// The recorded, on-disk and upgrade facts a file-integrity page carries.
    pub triage: &'a mut dyn FnMut(&ResultsRow) -> Option<OwnedTriage>,
}

/// A signing verdict, owned so it can outlive the call that produced it.
pub struct OwnedSigning {
    pub untrusted: bool,
    pub text: String,
}

/// The three display-only facts a file-integrity page carries.
#[derive(Default)]
pub struct OwnedTriage {
    pub recorded: String,
    pub ondisk: String,
    pub upgrade: String,
}

/// Judges one batch against the allowlist, the spool and the collaborators.
pub struct BatchJudge<'a> {
    pub home: &'a str,
    pub allowlist_path: &'a str,
    pub allowlist: Option<&'a AllowlistText>,
    pub spool: &'a DigestAppendFile,
    pub collaborators: Collaborators<'a>,
    pub now: &'a str,
}

impl JudgeFindings for BatchJudge<'_> {
    fn judge(&mut self, records: &str) -> JudgedBatch {
        let batch = rows(records);
        let entries = self.allowlist.map(AllowlistText::entries);
        // EVERY COLLABORATOR ASKED BEFORE ANY ROW IS JUDGED, so the answers
        // outlive the loop that borrows them. It also puts every spawned
        // inspection in one pass rather than interleaved with rendering, which
        // is what a later concurrent enricher would need.
        let evidence: Vec<(Option<OwnedSigning>, Option<OwnedTriage>)> = batch
            .iter()
            .map(|row| {
                (
                    (self.collaborators.inspect)(&row.enrichment_path),
                    (self.collaborators.triage)(row),
                )
            })
            .collect();
        let mut page_findings = Vec::new();
        for (row, (signing, triage)) in batch.iter().zip(&evidence) {
            match self.outcome(row, entries.as_deref(), signing.as_ref(), triage.as_ref()) {
                Outcome::Page { signing, triage } => {
                    page_findings.push(page_finding(row, signing, triage));
                }
                Outcome::Digest => self.spool_row(row),
                Outcome::Quiet => {}
            }
        }
        JudgedBatch {
            page: page(&page_findings),
        }
    }
}

/// The gate's answer, with the borrowed lifetimes resolved away so the loop can
/// hold it past the call.
enum Outcome<'a> {
    Page {
        signing: Option<&'a str>,
        triage: Option<Triage<'a>>,
    },
    Digest,
    Quiet,
}

impl BatchJudge<'_> {
    fn outcome<'a>(
        &mut self,
        row: &'a ResultsRow,
        entries: Option<&'a [posture_domain::AllowlistEntry<'a>]>,
        signing: Option<&'a OwnedSigning>,
        triage: Option<&'a OwnedTriage>,
    ) -> Outcome<'a> {
        let columns = row.gate_columns();
        let finding = GateFinding {
            detector: row.detector,
            action: row.action,
            enrichment_path: &row.enrichment_path,
            columns,
        };
        let evidence = GateEvidence {
            severity: Some(severity(row.detector, row.action, row.protection)),
            signing: signing.map(|signing| Signing {
                untrusted: signing.untrusted,
                text: &signing.text,
            }),
            // THE INTEGRITY ARM IS THE COLLABORATOR'S, asked only of a file the
            // manifest could vouch for. A row that is not a file event has
            // nothing to compare and stays at its detector's own tier.
            integrity: self.integrity(row),
            triage: triage.map(|triage| Triage {
                recorded: &triage.recorded,
                ondisk: &triage.ondisk,
                upgrade: &triage.upgrade,
            }),
        };
        let allowlisted = |identity: LaunchdIdentity<'_>| self.allowlisted(entries, identity);
        match gate(finding, evidence, allowlisted) {
            GateOutcome::Page { signing, triage } => Outcome::Page { signing, triage },
            GateOutcome::Digest { .. } => Outcome::Digest,
            GateOutcome::LogOnly => Outcome::Quiet,
        }
    }

    fn integrity(&mut self, row: &ResultsRow) -> IntegrityVerdict {
        let path = row.gate_columns().target_path;
        if path.is_empty() {
            return IntegrityVerdict::LogOnly;
        }
        if (self.collaborators.vouches)(path) {
            IntegrityVerdict::LogOnly
        } else {
            IntegrityVerdict::Page
        }
    }

    /// Whether the allowlist vouches for this launchd tuple.
    ///
    /// AN UNREADABLE LIST IS NOT AN EMPTY ONE. `None` here is the list that
    /// could not be read at all, and the domain turns that into a page rather
    /// than into a suppression that never happened.
    fn allowlisted(
        &mut self,
        entries: Option<&[posture_domain::AllowlistEntry<'_>]>,
        identity: LaunchdIdentity<'_>,
    ) -> bool {
        let list = match entries {
            Some(entries) => Allowlist::Read(entries),
            None => Allowlist::Unreadable,
        };
        let verdict = allowlist_verdict(
            self.home,
            self.allowlist_path,
            list,
            identity,
            None,
            |path| (self.collaborators.vouches)(path),
        );
        verdict == posture_domain::AllowlistVerdict::Suppress
    }

    fn spool_row(&self, row: &ResultsRow) {
        let detector = row.detector.query_name();
        self.spool.append(&posture_protocol::DigestRecord {
            timestamp: Some(self.now.to_string()),
            detector: Some(detector.to_string()),
            category: Some(row.column("category").to_string()),
            identity: Some(identity(row)),
            // THE ROW'S OWN ACTION, not a column of the same name. osquery
            // writes the differential verb beside the columns, and reading it
            // out of them spooled an empty action for every finding.
            action: Some(action(row).to_string()),
            summary: Some(format!("{detector} {}", named(row))),
        });
    }
}

/// The verb the row carried, as the digest prints it.
fn action(row: &ResultsRow) -> &'static str {
    match row.action {
        Action::Added => "added",
        Action::Removed => "removed",
        Action::Other => "changed",
    }
}

/// WHICH thing a finding is about.
///
/// A LISTENING PORT IS THREE FACTS, not one: the program alone does not say
/// what it exposed, and a digest line naming only `node` is a line nobody can
/// act on. Every other detector has one column that identifies it, tried in the
/// order the shell tried them.
fn identity(row: &ResultsRow) -> String {
    if row.detector == Detector::ListeningPortsNonLoopback {
        return format!(
            "{} {}:{}",
            first(row, &["name", "path"]),
            first(row, &["address"]),
            first(row, &["port"])
        );
    }
    named(row).to_string()
}

fn named(row: &ResultsRow) -> &str {
    first(
        row,
        &["label", "identifier", "target_path", "path", "username"],
    )
}

/// The first of these columns the row actually filled, or the shell's own
/// `"?"`, which is a placeholder a reader recognizes as one.
fn first<'a>(row: &'a ResultsRow, columns: &[&str]) -> &'a str {
    for column in columns {
        let value = row.column(column);
        if !value.is_empty() {
            return value;
        }
    }
    "?"
}

fn page_finding<'a>(
    row: &'a ResultsRow,
    signing: Option<&'a str>,
    triage: Option<Triage<'a>>,
) -> PageFinding<'a> {
    PageFinding {
        query: row.detector.query_name(),
        severity: Severity::Critical,
        enrichment_path: &row.enrichment_path,
        columns: row.page_columns(),
        act: Some(row.column("action")).filter(|action| !action.is_empty()),
        signing: signing.map(|text| Signing {
            untrusted: true,
            text,
        }),
        triage,
    }
}

/// NO FINDINGS IS NO PAGE, not a page saying zero. A batch whose every row was
/// spooled or dropped has nothing to wake anyone for.
fn page(findings: &[PageFinding<'_>]) -> Option<BatchPage> {
    if findings.is_empty() {
        return None;
    }
    let rendered = render_page(findings);
    if rendered.count == 0 {
        return None;
    }
    Some(BatchPage {
        title: if rendered.count > 1 {
            format!("🔴 **CRITICAL** · {}", rendered.count)
        } else {
            "🔴 **CRITICAL**".to_string()
        },
        body: rendered.body,
    })
}

#[cfg(test)]
#[path = "judge_batch/tests.rs"]
mod tests;
