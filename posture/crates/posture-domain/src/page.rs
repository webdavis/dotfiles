//! The critical page: what the operator reads on a phone, and its caps.
//!
//! One thing at a time, glanceable, minimal fields, ending in a single action.
//! Only critical findings reach it; everything else is already in the log and
//! the daily digest, and a page that carries the merely interesting teaches the
//! operator to dismiss the one that matters.
//!
//! DISPLAY ONLY. Nothing here changes what was detected or how it was tiered.

mod fields;
mod header;
mod next_step;

use crate::{Detector, Severity, Signing, Triage};

/// How many blocks the page renders before it stops and says how many it left.
///
/// The cap bounds COUNT, which a simultaneous batch of critical findings would
/// otherwise blow through, and the marker is what keeps the page honest about
/// what it is not showing.
pub const BLOCK_LIMIT: usize = 8;

/// The whole body's hard cap, below the delivery limit it protects.
///
/// THE BLOCK CAP IS NOT ENOUGH ON ITS OWN: eight blocks of long fields still
/// exceed two thousand characters, and an over-length page does not arrive
/// truncated, it wedges the spool undelivered. This is the backstop that makes
/// that impossible.
pub const BODY_LIMIT: usize = 1900;

/// What marks a body cut short.
const BODY_TRUNCATION: &str = "\n… (truncated to fit the 2000-char limit - see results.log)";

/// The columns a page may render, each absent when the row did not carry it.
///
/// `Option` rather than an empty string, because the two differ: a row carrying
/// an empty label HAS a label, and the identifier fallback must stop there
/// rather than walk on to the next column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageColumns<'a> {
    pub label: Option<&'a str>,
    pub program: Option<&'a str>,
    pub name: Option<&'a str>,
    pub command: Option<&'a str>,
    pub path: Option<&'a str>,
    pub username: Option<&'a str>,
    pub uid: Option<&'a str>,
    pub address: Option<&'a str>,
    pub port: Option<&'a str>,
    pub service: Option<&'a str>,
    pub identifier: Option<&'a str>,
    pub team: Option<&'a str>,
    pub target_path: Option<&'a str>,
    pub category: Option<&'a str>,
    pub action: Option<&'a str>,
    pub filename: Option<&'a str>,
    pub dest_filename: Option<&'a str>,
}

/// One enriched finding, as the page reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFinding<'a> {
    /// The detector's own name. THE ONE SOURCE OF TRUTH for which detector this
    /// is: the mapped variant is derived from it rather than carried beside it,
    /// so the two cannot disagree, and an unmapped name still renders.
    pub query: &'a str,
    pub severity: Severity,
    /// The path the enricher resolved, which the next step offers to inspect.
    pub enrichment_path: &'a str,
    pub columns: PageColumns<'a>,
    /// The action, when the row carried it beside the columns rather than in
    /// them.
    pub act: Option<&'a str>,
    pub signing: Option<Signing<'a>>,
    pub triage: Option<Triage<'a>>,
}

impl<'a> PageFinding<'a> {
    fn detector(&self) -> Option<Detector> {
        Detector::from_query(self.query)
    }
}

/// A rendered page: how many critical findings there were, and the body.
///
/// The count is every one of them, including the ones the caps left out, so the
/// number the operator sees is the number that happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub count: usize,
    pub body: String,
}

/// Render the critical findings into one page.
pub fn render_page(findings: &[PageFinding<'_>]) -> Page {
    let critical: Vec<&PageFinding<'_>> = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Critical)
        .collect();
    let mut body = critical
        .iter()
        .take(BLOCK_LIMIT)
        .map(|finding| block(finding))
        .collect::<Vec<String>>()
        .join("\n\n");
    if let Some(dropped) = critical.len().checked_sub(BLOCK_LIMIT).filter(|n| *n > 0) {
        body.push_str(&format!(
            "\n\n… and {dropped} more CRITICAL finding(s) - see results.log"
        ));
    }
    Page {
        count: critical.len(),
        body: capped(body),
    }
}

/// One finding's block: header, decision fields, next step.
fn block(finding: &PageFinding<'_>) -> String {
    let mut lines = vec![format!("**{}**", header::header(finding))];
    lines.extend(fields::fields(finding));
    lines.extend(next_step::next_step(finding));
    lines.join("\n")
}

/// The body, cut to `BODY_LIMIT` characters if it overruns.
///
/// CHARACTERS, not bytes, because a cut through a multi-byte character renders
/// a replacement glyph where the operator expects a path.
fn capped(body: String) -> String {
    if body.chars().count() <= BODY_LIMIT {
        return body;
    }
    let mut cut: String = body.chars().take(BODY_LIMIT).collect();
    cut.push_str(BODY_TRUNCATION);
    cut
}

#[cfg(test)]
mod tests;
