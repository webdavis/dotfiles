//! The cargo lane: every crate `cargo install` put on this machine, checked
//! against the registry weekly.
//!
//! IT REPORTS BY DEFAULT AND DOES NOT COMPILE. A `cargo install` builds from
//! source, which is minutes per crate on an unattended weekly run, so the lane
//! tells the operator what is behind and hands them the exact command. Turning
//! `compile` on in the config is what makes it do the work instead.
//!
//! A GIT-SOURCED CRATE IS NAMED AND SKIPPED. crates.io has no version of it to
//! compare against; a newer revision would come from its own remote, which this
//! lane does not fetch. Saying so in the record is the point: silence would
//! read as "up to date".
//!
//! ONE SEARCH PER CRATE, and a search that fails costs that crate alone. The
//! registry answering slowly for one name is not a reason to stop checking the
//! rest.

mod listing;

use crate::config::CargoLane;
use crate::lanes::{CommandRunner, LaneAdapter};
use listing::{Installed, Source, behind_sentence, parse_install_list, parse_search};
use uu_domain::{LaneReport, RunFacts};

impl LaneAdapter for CargoLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_cargo_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let cargo = self.cargo.as_str();
        let stdout = match runner.run(cargo, &["install", "--list"]) {
            Ok(stdout) => stdout,
            Err(why) => {
                // WITHOUT THE LISTING THERE IS NO LANE. Reporting zero crates
                // behind would be indistinguishable from every crate current.
                report.failed(format!("{cargo} install --list FAILED ({why})"));
                return report;
            }
        };
        for installed in parse_install_list(&stdout) {
            self.check(&installed, cargo, runner, &mut report);
        }
        report
    }
}

impl CargoLane {
    fn check(
        &self,
        installed: &Installed,
        cargo: &str,
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) {
        if let Source::Git { rev } = &installed.source {
            report.noted(format!(
                "{} {}: installed from git at {rev}, which the registry cannot answer for",
                installed.name, installed.version
            ));
            return;
        }
        let newest = match runner.run(cargo, &["search", &installed.name, "--limit", "1"]) {
            Ok(stdout) => parse_search(&stdout, &installed.name),
            Err(why) => {
                report.failed(format!("{} search FAILED ({why})", installed.name));
                return;
            }
        };
        let Some(newest) = newest else {
            // The search ran and the crate was not in its answer. That is a
            // fact about the registry, not a failure of this machine.
            report.noted(format!(
                "{} {}: the registry named no version for it",
                installed.name, installed.version
            ));
            return;
        };
        if newest == installed.version {
            report.noted(format!("{} {}: current", installed.name, installed.version));
            return;
        }
        self.behind(installed, &newest, cargo, runner, report);
    }

    /// What to do about a crate the registry has moved past.
    ///
    /// VERSIONS ARE COMPARED FOR EQUALITY, never ordered. A crate installed
    /// AHEAD of the registry, from a local path or a yanked release, is
    /// reported as behind, which is a wrong word for a true fact: the two
    /// disagree. Ordering them would need semver parsing to say something the
    /// operator can already see from both numbers being printed.
    fn behind(
        &self,
        installed: &Installed,
        newest: &str,
        cargo: &str,
        runner: &dyn CommandRunner,
        report: &mut LaneReport,
    ) {
        if !self.compile {
            report.pending(behind_sentence(installed, newest));
            return;
        }
        match runner.run(cargo, &["install", &installed.name]) {
            Ok(_) => report.noted(format!(
                "{}: compiled {} → {newest}",
                installed.name, installed.version
            )),
            // ONE FAILED BUILD DOES NOT STOP THE NEXT CRATE. A crate that needs
            // a toolchain this machine lacks would otherwise hide every crate
            // listed after it.
            Err(why) => report.failed(format!(
                "{}: compiling {} → {newest} FAILED ({why})",
                installed.name, installed.version
            )),
        }
    }
}

#[cfg(test)]
#[path = "cargo/tests.rs"]
mod tests;
