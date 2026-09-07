//! The herdr lane: the binary refreshes itself, then every plugin in the
//! roster is reinstalled at its source's tip.
//!
//! herdr HAS NO `plugin update`, so a refresh is an uninstall followed by a
//! fresh install, which re-pins at the source's tip. A failed install RETRIES
//! ONCE and a plugin that still failed is named loudly, so the record for the
//! week says exactly what is missing. The running server keeps its loaded
//! plugins until restart, so a failure costs the next restart rather than the
//! current session.
//!
//! THE RUN EVENT IS UNUSED HERE: this lane predates it and drives herdr by
//! argv alone.

use crate::config::HerdrLane;
use crate::lanes::{CommandRunner, LaneAdapter};
use uu_domain::LaneReport;
use uu_domain::RunFacts;

impl LaneAdapter for HerdrLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_herdr_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);

        match runner.run(&self.binary, &["update"]) {
            Ok(_) => {
                // The version is a COURTESY in the record and never a verdict:
                // a build that will not print its own version still updated.
                let version = runner
                    .run(&self.binary, &["--version"])
                    .ok()
                    .and_then(|out| out.lines().next().map(str::to_string))
                    .filter(|line| !line.is_empty());
                report.noted(match version {
                    Some(version) => format!("herdr self-update: ok ({version})"),
                    None => "herdr self-update: ok".to_string(),
                });
            }
            Err(why) => report.failed(format!(
                "herdr self-update FAILED ({why}); plugins still refresh below"
            )),
        }

        for plugin in &self.plugins {
            let id = plugin.id.as_str();
            // AN INSTALL OVER A FAILED UNINSTALL IS NOT ATTEMPTED. herdr pins
            // a plugin at install, so installing on top of a copy that would
            // not come off is how one plugin becomes two.
            if let Err(why) = runner.run(&self.binary, &["plugin", "uninstall", id]) {
                report.failed(format!(
                    "plugin {id}: uninstall failed ({why}); leaving the installed copy alone"
                ));
                continue;
            }
            let install =
                || runner.run(&self.binary, &["plugin", "install", &plugin.repo, "--yes"]);
            // The retry is the SECOND call and there is no third: `or_else`
            // runs it only on a failure, and the reason kept is the one the
            // last attempt gave.
            match install().or_else(|_| install()) {
                Ok(_) => report.noted(format!("plugin {id}: refreshed")),
                Err(why) => report.failed(format!(
                    "plugin {id}: REINSTALL FAILED twice ({why}); it is now MISSING until the \
                     next apply or run"
                )),
            }
        }
        report
    }
}

#[cfg(test)]
mod tests;
