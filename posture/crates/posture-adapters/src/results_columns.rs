//! A row's columns, projected into the two typed views the domain renders and
//! gates on.
//!
//! ABSENT AND EMPTY ARE THE SAME ANSWER, and that is the whole reason this is a
//! projection rather than a field-by-field copy. osquery writes a column it has
//! no value for as an empty string, and the page renders `Some("")` as a
//! labelled row with nothing after the label. Every column here is therefore
//! read through one helper that answers `None` for both.
//!
//! THE PROJECTION BORROWS, it does not own. `PageColumns` is a struct of
//! `&str`, so this hands back views into the row's own JSON and the row has to
//! outlive them. That is what keeps a page render allocation-free over a batch
//! that can carry hundreds of findings.

use super::ResultsRow;
use posture_domain::{FileCategory, GateColumns, LaunchdIdentity, PageColumns};

impl ResultsRow {
    /// One column, or `None` when the row has nothing to say for it.
    fn text(&self, name: &str) -> Option<&str> {
        Some(self.column(name)).filter(|value| !value.is_empty())
    }

    /// The columns as the page reads them.
    pub fn page_columns(&self) -> PageColumns<'_> {
        PageColumns {
            label: self.text("label"),
            program: self.text("program"),
            name: self.text("name"),
            command: self.text("command"),
            path: self.text("path"),
            username: self.text("username"),
            uid: self.text("uid"),
            address: self.text("address"),
            port: self.text("port"),
            service: self.text("service"),
            identifier: self.text("identifier"),
            team: self.text("team"),
            target_path: self.text("target_path"),
            category: self.text("category"),
            action: self.text("action"),
            filename: self.text("filename"),
            dest_filename: self.text("dest_filename"),
        }
    }

    /// The columns the gate matches on.
    ///
    /// THE LAUNCHD TUPLE IS ALL THREE OR IT IS NOTHING. An allowlist entry
    /// matches on label, canonical path and program together, so a row that
    /// carried only a label must not match an entry that named all three: the
    /// empty fields stay empty and the comparison fails, which is the safe
    /// direction.
    pub fn gate_columns(&self) -> GateColumns<'_> {
        GateColumns {
            launchd: LaunchdIdentity {
                label: self.column("label"),
                path: self.column("path"),
                program: self.column("program"),
            },
            file_category: self.file_category(),
            // THE TARGET PATH FALLS BACK TO `path`, because the file detectors
            // do not agree on which column carries the file: a file event names
            // `target_path` and a launchd or suid row names `path`, and the gate
            // asks one question of all of them.
            target_path: self
                .text("target_path")
                .unwrap_or_else(|| self.column("path")),
        }
    }

    /// Which watched tree this row's file sits in.
    ///
    /// IT IS OSQUERY'S OWN WORD, not a path this reads. The file-integrity
    /// config names each watched tree, and osquery stamps that name onto every
    /// event it emits from it. Re-deriving the tree from the path here would be
    /// a SECOND copy of the config's own boundaries, free to drift from the one
    /// that actually decided which files are watched, and the drift would be
    /// silent: a file would keep being watched while the alerter judged it under
    /// the wrong rules.
    fn file_category(&self) -> FileCategory {
        match self.column("category") {
            "ssh" => FileCategory::Ssh,
            "sshd_config" => FileCategory::SshdConfig,
            "pipeline_integrity" => FileCategory::PipelineIntegrity,
            "managed_bin" => FileCategory::ManagedBin,
            "launch_agents" => FileCategory::LaunchAgents,
            "launch_daemons" => FileCategory::LaunchDaemons,
            "allowlist_file" => FileCategory::AllowlistFile,
            "sudoers" => FileCategory::Sudoers,
            // A CATEGORY THIS DOES NOT KNOW IS NOT A REFUSAL. Adding a watched
            // tree to the osquery config must not make its events unjudgeable;
            // `Other` is the tier every unnamed tree already had.
            _ => FileCategory::Other,
        }
    }
}

#[cfg(test)]
#[path = "results_columns/tests.rs"]
mod tests;
