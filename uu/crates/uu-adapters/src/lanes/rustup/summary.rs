//! Reading the block `rustup update` prints when it has finished.
//!
//! Its shape, captured from this machine on 2026-09-09, is an indented line per
//! toolchain plus one for rustup itself when it updated:
//!
//! ```text
//!   stable-aarch64-apple-darwin unchanged - rustc 1.98.1 (48a229cea 2026-09-01)
//!    nightly-aarch64-apple-darwin updated - rustc 1.100.0-nightly (a36d05efa 2026-09-09) (from rustc 1.92.0-nightly (0be8e1608 2025-09-19))
//! ```
//!
//! The names are right-aligned, so the indent varies and carries no meaning.
//! Every other line rustup prints is an `info:` narration this ignores.

/// What became of one toolchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Updated { from: String, to: String },
    Unchanged(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Toolchain {
    pub(super) name: String,
    pub(super) outcome: Outcome,
}

/// The separator between a toolchain's name-and-verb and its version.
const DASH: &str = " - ";

/// Read the summary block, skipping every line that is not one of its rows.
pub(super) fn parse_update_summary(stdout: &str) -> Vec<Toolchain> {
    stdout.lines().filter_map(row).collect()
}

fn row(line: &str) -> Option<Toolchain> {
    let (head, version) = line.trim().split_once(DASH)?;
    let (name, verb) = head.rsplit_once(' ')?;
    if name.is_empty() || version.is_empty() {
        return None;
    }
    let outcome = match verb {
        "unchanged" => Outcome::Unchanged(version.to_string()),
        "updated" => updated(version)?,
        // A VERB THIS DOES NOT KNOW IS NOT A ROW. rustup could add one, and
        // reading it as either of these two would state the opposite of
        // whatever it means half the time.
        _ => return None,
    };
    Some(Toolchain {
        name: name.to_string(),
        outcome,
    })
}

/// `<to> (from <from>)`, where both halves carry parentheses of their own, so
/// the split is on the marker rather than on the first `(`.
fn updated(version: &str) -> Option<Outcome> {
    let (to, from) = version.split_once(" (from ")?;
    let from = from.strip_suffix(')')?;
    if to.is_empty() || from.is_empty() {
        return None;
    }
    Some(Outcome::Updated {
        from: from.to_string(),
        to: to.to_string(),
    })
}

#[cfg(test)]
#[path = "summary/tests.rs"]
mod tests;
