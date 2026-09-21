//! The uv lane: every tool `uv` installed, upgraded in place, weekly.
//!
//! ONE COMMAND AND NO ROSTER. `uv tool upgrade --all` already walks every
//! installed tool, so the whole lane is that call, the line it leaves in the
//! record, and the exit code as the verdict.
//!
//! WHAT IS INSTALLED IS REPORTED AGAINST WHAT IS DECLARED, when the config
//! states a `declared` roster. Nothing is ever removed: the extra names are
//! read off `uv tool list` and noted, one line each, so a tool that arrived by
//! hand shows up instead of living on unrecorded.
//!
//! AN ABSENT `uv` IS A FAILURE HERE, deliberately unlike the bash weekly job
//! this ports from, which printed "nothing to upgrade" and returned clean when
//! the binary was missing. A machine that declares the lane and has no uv is a
//! machine whose tools stopped being upgraded, and a record that reads the same
//! either way is how that goes unnoticed for months.

use crate::config::UvLane;
use crate::lanes::undeclared::note_undeclared;
use crate::lanes::{CommandRunner, LaneAdapter};
use uu_domain::LaneReport;
use uu_domain::RunFacts;

/// The arguments the lane always runs `uv` with.
const UPGRADE: [&str; 3] = ["tool", "upgrade", "--all"];

/// The listing the report reads.
const LISTING: [&str; 2] = ["tool", "list"];

/// Upgrade every uv tool, and report what that took.
impl LaneAdapter for UvLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_uv_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let binary = self.binary.as_str();
        match runner.run(binary, &UPGRADE) {
            // uv NARRATES ON STDERR, which a clean run drops, so this line is
            // what the record has to say the lane ran at all.
            Ok(_) => report.noted(format!("{binary} tool upgrade --all: ok")),
            Err(why) => report.failed(format!("{binary} tool upgrade --all FAILED ({why})")),
        }
        if let Some(declared) = &self.declared {
            self.report_undeclared(declared, runner, &mut report);
        }
        report
    }
}

impl UvLane {
    /// List what is installed and note whatever the roster does not declare.
    fn report_undeclared(
        &self,
        declared: &[String],
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) {
        let binary = self.binary.as_str();
        match runner.run(binary, &LISTING) {
            Ok(stdout) => note_undeclared(report, installed(&stdout), declared),
            // WITHOUT THE LISTING THERE IS NO REPORT, and a silent lane reads
            // exactly like a machine with nothing undeclared on it.
            Err(why) => report.failed(format!("{binary} tool list FAILED ({why})")),
        }
    }
}

/// The tools in a `uv tool list` answer: one `<name> v<version>` line each,
/// with every executable the tool installed indented beneath it.
///
/// AN EXECUTABLE LINE IS NOT A TOOL: it is a `- <name>` entry under the tool
/// that installed it. And a line is a tool only if its second word is a `v`
/// version, so a sentence uv prints in place of a list contributes no name.
fn installed(stdout: &str) -> Vec<(&str, &str)> {
    stdout
        .lines()
        .filter(|line| !line.starts_with('-') && !line.starts_with(char::is_whitespace))
        .filter_map(|line| line.split_once(' '))
        .filter_map(|(name, rest)| {
            let at = rest.trim().strip_prefix('v')?;
            (!name.is_empty() && !at.is_empty()).then_some((name, at))
        })
        .collect()
}

#[cfg(test)]
#[path = "uv/tests.rs"]
mod tests;
