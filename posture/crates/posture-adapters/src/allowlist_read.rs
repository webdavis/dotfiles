//! The launchd page-allowlist, as the alerter reads it.
//!
//! The writer publishes this file and the judge consults it; they meet at the
//! line format and nowhere else. One NDJSON object per line, each naming a
//! label, a path, a program and optionally a sha256 pinning the plist's bytes.
//!
//! PATHS ARE STORED HOME-RELATIVE. The committed seed file has to be
//! user-agnostic, so a stored `~/Library/...` is expanded here, before the
//! domain compares it to the finding's absolute path. Expanding in the domain
//! instead would put `$HOME` in a crate that is not allowed to read the
//! environment at all.
//!
//! UNREADABLE IS NOT EMPTY, and the distinction is the whole security posture
//! of this file. An allowlist ENTRY suppresses a persistence page, so a list
//! this cannot read must not read as a list with nothing in it: that would
//! suppress nothing, which is correct, but it would also say the list was
//! consulted, which is a lie. `Allowlist::Unreadable` is the arm the domain
//! turns into a page.

use posture_domain::{AllowlistEntry, LaunchdIdentity};
use serde_json::Value;
use std::path::Path;

/// The lines of an allowlist, owned so the borrowed entries can point into them.
#[derive(Debug, Default)]
pub struct AllowlistText {
    entries: Vec<OwnedEntry>,
}

#[derive(Debug)]
struct OwnedEntry {
    label: String,
    path: String,
    program: String,
    sha256: String,
}

impl AllowlistText {
    /// Read the file at `path`, expanding a leading `~/` against `home`.
    ///
    /// `None` when the file cannot be read at all, which the caller turns into
    /// `Allowlist::Unreadable`. A file that reads but holds nothing usable is
    /// an EMPTY allowlist, not an unreadable one: it was consulted and vouched
    /// for nothing.
    pub fn read(path: &Path, home: &str) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        Some(Self::parse(&text, home))
    }

    pub fn parse(text: &str, home: &str) -> Self {
        Self {
            entries: text.lines().filter_map(|line| entry(line, home)).collect(),
        }
    }

    /// The entries, as the domain compares them.
    pub fn entries(&self) -> Vec<AllowlistEntry<'_>> {
        self.entries
            .iter()
            .map(|entry| AllowlistEntry {
                identity: LaunchdIdentity {
                    label: &entry.label,
                    path: &entry.path,
                    program: &entry.program,
                },
                sha256: &entry.sha256,
            })
            .collect()
    }
}

fn entry(line: &str, home: &str) -> Option<OwnedEntry> {
    let value: Value = serde_json::from_str(line).ok()?;
    let text = |name: &str| value.get(name).and_then(Value::as_str).unwrap_or_default();
    let label = text("label");
    // A TUPLE WITHOUT A LABEL VOUCHES FOR NOTHING, and keeping it would put an
    // entry in the list whose empty label matches a finding whose label osquery
    // failed to read. Two absences are not an agreement.
    if label.is_empty() {
        return None;
    }
    Some(OwnedEntry {
        label: label.to_string(),
        path: expand_home(text("path"), home),
        program: expand_home(text("program"), home),
        sha256: text("sha256").to_string(),
    })
}

/// A leading `~/` becomes `$HOME/`.
///
/// A DELIBERATE NARROWING FROM THE SHELL, named because it is one. The bash
/// wrote `${value//~\//$HOME/}`, which replaces EVERY `~/` in the string, while
/// the comment above it said "expand a leading ~/". The comment is the intent
/// and the code was the accident: a mid-string expansion turns a real path that
/// happens to contain `~/` into a different path, and in a file whose entries
/// SUPPRESS a page, a path-confusion bug points the wrong way. This does what
/// the comment said. The narrowing can only ever make an entry match less, so
/// its failure mode is a page that fires, never one that is silenced.
fn expand_home(value: &str, home: &str) -> String {
    match value.strip_prefix("~/") {
        Some(rest) if !home.is_empty() => format!("{home}/{rest}"),
        _ => value.to_string(),
    }
}

#[cfg(test)]
#[path = "allowlist_read/tests.rs"]
mod tests;
