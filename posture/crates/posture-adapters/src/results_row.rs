//! One osquery results-log line, read as the finding the domain judges.
//!
//! THE WIRE SHAPE LIVES HERE AND THE POLICY DOES NOT. Which query names count,
//! what a baseline row is, and which column an enricher should inspect are all
//! `posture-domain` answers (`Detector::from_query`, `keeps`,
//! `enrichment_path`); this module's whole job is turning bytes into the
//! arguments they take.
//!
//! A LINE THAT WILL NOT PARSE COSTS ONE LINE. osquery can be killed mid-write
//! and something else can append to the log, so a batch is expected to carry
//! rubbish now and then. Refusing the batch over one row would drop every
//! finding beside it, and those are the rows this exists to page about.

use posture_domain::{Action, Detector, EnrichmentPaths, ProtectionState};
use serde_json::Value;

/// A results row this pipeline judges, with the wire's shapes already resolved.
pub struct ResultsRow {
    pub detector: Detector,
    pub action: Action,
    pub protection: ProtectionState,
    pub enrichment_path: String,
    pub columns: Value,
}

impl ResultsRow {
    /// One column, or the empty string. Absent, null and a non-string all read
    /// the same way, because the alerter this replaces read every column
    /// through `// ""` and no caller distinguishes them.
    pub fn column(&self, name: &str) -> &str {
        self.columns.get(name).and_then(Value::as_str).unwrap_or("")
    }
}

/// Every row in a batch that survives the domain's own admission rules.
///
/// The order of the surviving rows is the order they arrived in, because a page
/// reads as a timeline and osquery already wrote them in the order it saw them.
pub fn rows(records: &str) -> Vec<ResultsRow> {
    records.lines().filter_map(row).collect()
}

fn row(line: &str) -> Option<ResultsRow> {
    let value: Value = serde_json::from_str(line).ok()?;
    let detector = Detector::from_query(value.get("name")?.as_str()?)?;
    let columns = value.get("columns").cloned().unwrap_or(Value::Null);
    let text = |name: &str| -> &str {
        columns
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
    };
    // AN ABSENT COUNTER IS NOT A BASELINE. osquery writes `counter` on a
    // differential row and omits it elsewhere, and reading the absence as zero
    // would seed away a first observation the domain means to page about.
    let counter_is_zero = value.get("counter").and_then(counter) == Some(0);
    if !detector.keeps(counter_is_zero, text("target_path")) {
        return None;
    }
    let enrichment_path = detector.enrichment_path(EnrichmentPaths {
        path: text("path"),
        target_path: text("target_path"),
        // AN EMPTY BUNDLE PATH IS NO BUNDLE PATH, so the domain falls back to
        // `path` for it. jq read the shell's `//` that way and the domain's
        // `unwrap_or` cannot, since `Some("")` is a value.
        bundle_path: columns
            .get("bundle_path")
            .and_then(Value::as_str)
            .filter(|bundle| !bundle.is_empty()),
    });
    Some(ResultsRow {
        detector,
        // A ROW WITHOUT AN ACTION IS A CHANGE, which is the default the shell
        // spelled `.action // "changed"`, and "changed" is neither added nor
        // removed.
        action: match value.get("action").and_then(Value::as_str) {
            Some("added") => Action::Added,
            Some("removed") => Action::Removed,
            _ => Action::Other,
        },
        // THE ONE COLUMN SEVERITY READS. A protection detector carries the
        // state it found here, and anything that is not the word `off` leaves
        // the row at its detector's ordinary tier.
        protection: match text("state") {
            "off" => ProtectionState::Off,
            _ => ProtectionState::Other,
        },
        enrichment_path,
        columns,
    })
}

/// osquery writes the counter as a number, and its own JSON logger has written
/// it as a quoted number in the past. Both are the same counter.
fn counter(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "results_row/tests.rs"]
mod tests;
