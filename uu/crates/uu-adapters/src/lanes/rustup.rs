//! The rustup lane: every installed toolchain, updated weekly.
//!
//! ONE COMMAND AND NO ROSTER. `rustup update` already walks every toolchain
//! this machine has, so the lane is that call plus reading the summary it
//! prints at the end.
//!
//! THE SUMMARY IS READ, not just the exit code, because rustup exits 0 whether
//! it moved a toolchain or found nothing to do. A record that said only "ok"
//! could not answer the question the lane exists for, which is what changed.
//!
//! A LINE OF ANOTHER SHAPE IS SKIPPED, never half read. rustup's output is
//! human text with no machine format behind it, and guessing at a line means
//! recording a version nobody is running.

mod summary;

use crate::config::RustupLane;
use crate::lanes::{CommandRunner, LaneAdapter};
use summary::{Outcome, parse_update_summary};
use uu_domain::{LaneReport, RunFacts};

impl LaneAdapter for RustupLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_rustup_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let rustup = self.rustup.as_str();
        let stdout = match runner.run(rustup, &["update"]) {
            Ok(stdout) => stdout,
            Err(why) => {
                report.failed(format!("{rustup} update FAILED ({why})"));
                return report;
            }
        };
        let toolchains = parse_update_summary(&stdout);
        if toolchains.is_empty() {
            // IT RAN AND SAID NOTHING THIS COULD READ. Recording that is the
            // honest answer; claiming every toolchain is current would be a
            // fact this never established.
            report.noted(format!(
                "{rustup} update: ran, and named no toolchain this could read"
            ));
            return report;
        }
        for toolchain in toolchains {
            report.noted(match toolchain.outcome {
                Outcome::Updated { from, to } => {
                    format!("{}: {from} → {to}", toolchain.name)
                }
                Outcome::Unchanged(version) => format!("{}: {version}, current", toolchain.name),
            });
        }
        report
    }
}

#[cfg(test)]
#[path = "rustup/tests.rs"]
mod tests;
