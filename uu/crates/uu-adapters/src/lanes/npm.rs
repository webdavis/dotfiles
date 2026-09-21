//! The npm lane: every globally installed npm package upgraded, weekly, on
//! FNM'S NODE.
//!
//! THE PATH IS THE WHOLE PROBLEM. npm is an `#!/usr/bin/env node` script, so
//! whichever node PATH answers with is the node it runs on and the prefix it
//! installs into. Point the lane at fnm's npm and run it with some other
//! node's directory first and the upgrade lands in that other node's prefix,
//! silently. So the child runs with the directory npm itself sits in AHEAD of
//! everything uu inherited, which is the same dir fnm's node sits in. The
//! lane names that directory and the spawn seam joins it to the inherited
//! value, which uu's own process is the only place to read.
//!
//! WHAT IS INSTALLED IS REPORTED AGAINST WHAT IS DECLARED, when the config
//! states a `declared` roster. Nothing is ever removed: the extra names are
//! read off `npm ls -g` and noted, one line each, so a package that arrived by
//! hand shows up instead of living on unrecorded.
//!
//! AN ABSENT npm IS A FAILURE HERE, deliberately unlike the bash weekly job
//! this ports from, which printed "nothing to upgrade" and returned clean when
//! the binary was missing. A machine that declares the lane and has no npm at
//! the path it declared is a machine whose global packages stopped being
//! upgraded, and the record is where that has to show up.

use crate::config::NpmLane;
use crate::lanes::undeclared::note_undeclared;
use crate::lanes::{CommandRunner, Environment, LaneAdapter};
use uu_domain::LaneReport;
use uu_domain::RunFacts;

/// Upgrade every global npm package, and report what that took.
impl LaneAdapter for NpmLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_npm_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let binary = self.binary.as_str();
        match runner.run_in(
            binary,
            &["update", "-g"],
            &Environment::inheriting().prepending_path(bin_dir(binary)),
            None,
        ) {
            // npm narrates its upgrades on stdout, but a week with nothing to
            // upgrade prints nothing at all, so this line is what says the lane
            // ran.
            Ok(_) => report.noted(format!("{binary} update -g: ok")),
            Err(why) => report.failed(format!("{binary} update -g FAILED ({why})")),
        }
        if let Some(declared) = &self.declared {
            self.report_undeclared(declared, runner, &mut report);
        }
        report
    }
}

/// The listing the report reads: top level only, as JSON, so the names come
/// out of a document rather than out of a drawn tree.
const LISTING: [&str; 4] = ["ls", "-g", "--depth=0", "--json"];

/// What node itself brings. Both are in `npm ls -g` on an fnm node and neither
/// is ever declared, so reporting them would put the same two lines in every
/// weekly record forever.
const BUNDLED: [&str; 2] = ["corepack", "npm"];

impl NpmLane {
    /// List what is installed and note whatever the roster does not declare.
    ///
    /// The listing runs in the same environment the upgrade did, because it is
    /// the same npm and the same node prefix that has to answer.
    fn report_undeclared(
        &self,
        declared: &[String],
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) {
        let binary = self.binary.as_str();
        // `npm ls -g` exits non-zero whenever the tree has a problem (an
        // unmet peer dependency is enough) while still printing a complete,
        // parseable document on stdout, so the listing is read through the
        // seam that keeps a child's stdout past a non-clean exit rather than
        // the one that turns it into an `Err`.
        let listing = runner.run_reporting_in(
            binary,
            &LISTING,
            &Environment::inheriting().prepending_path(bin_dir(binary)),
        );
        let stdout = match listing {
            Ok(ran) => ran.stdout,
            // WITHOUT THE LISTING THERE IS NO REPORT, and a silent lane reads
            // exactly like a machine with nothing undeclared on it.
            Err(why) => {
                report.failed(format!("{binary} ls -g FAILED ({why})"));
                return;
            }
        };
        match installed(&stdout) {
            Ok(installed) => note_undeclared(
                report,
                installed
                    .iter()
                    .map(|(name, at)| (name.as_str(), at.as_str())),
                declared,
            ),
            Err(why) => report.failed(format!("{binary} ls -g answered unreadably ({why})")),
        }
    }
}

/// The global packages in an `npm ls -g --json` document: every key of
/// `dependencies` with the version beside it, minus what node bundles.
///
/// A document with no `dependencies` is a machine with nothing installed
/// rather than an answer this cannot read.
fn installed(stdout: &str) -> Result<Vec<(String, String)>, String> {
    let document: serde_json::Value =
        serde_json::from_str(stdout).map_err(|why| why.to_string())?;
    let Some(dependencies) = document
        .get("dependencies")
        .and_then(|value| value.as_object())
    else {
        return Ok(Vec::new());
    };
    Ok(dependencies
        .iter()
        .filter(|(name, _)| !BUNDLED.contains(&name.as_str()))
        .map(|(name, body)| {
            let at = body
                .get("version")
                .and_then(|value| value.as_str())
                // npm omits the version of a package whose directory it could
                // not read, and the name is still worth the line.
                .unwrap_or("unstated");
            (name.clone(), at.to_string())
        })
        .collect())
}

/// The directory `binary` sits in, which is what goes first on the child's
/// PATH. The config refuses anything but an absolute path, so there is always
/// a directory to name; a binary directly under the root names the root
/// itself rather than the empty string, which PATH reads as the working
/// directory.
fn bin_dir(binary: &str) -> &str {
    match binary.rsplit_once('/') {
        Some(("", _)) => "/",
        Some((dir, _)) => dir,
        None => ".",
    }
}

#[cfg(test)]
#[path = "npm/tests.rs"]
mod tests;
