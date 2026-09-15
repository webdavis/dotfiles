//! The herdr lane: the binary refreshes itself, then every plugin in the
//! roster is reinstalled, at its source's tip or at the revision it is pinned
//! to.
//!
//! herdr HAS NO `plugin update`, so a refresh is an uninstall followed by a
//! fresh install. An entry with no `ref` re-pins at the source's tip, which is
//! what every entry has always done. An entry WITH a `ref` is installed
//! through `herdr plugin install --ref <REF>` and does not move, and the
//! record says it was HELD rather than refreshed, so a pin cannot quietly
//! become a freeze nobody remembers taking.
//!
//! A REF THAT DOES NOT RESOLVE FAILS THE STEP. There is no tip fallback: an
//! install from tip after a pinned install failed would leave the record
//! claiming a revision the plugin is not at, which is the one outcome worse
//! than the plugin being missing.
//!
//! A failed install RETRIES ONCE and a plugin that still failed is named
//! loudly, so the record for the week says exactly what is missing. The
//! running server keeps its loaded plugins until restart, so a failure costs
//! the next restart rather than the current session.
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
            let pinned = plugin.pinned_ref.as_deref();
            let mut args = vec!["plugin", "install", plugin.repo.as_str()];
            if let Some(reference) = pinned {
                args.extend_from_slice(&["--ref", reference]);
            }
            args.push("--yes");
            let install = || runner.run(&self.binary, &args);
            // The retry is the SECOND call and there is no third: `or_else`
            // runs it only on a failure, and the reason kept is the one the
            // last attempt gave. BOTH ATTEMPTS CARRY THE SAME `--ref`.
            match install().or_else(|_| install()) {
                Ok(_) => report.noted(match pinned {
                    Some(reference) => {
                        format!("plugin {id}: HELD at {reference} (pinned, not updated)")
                    }
                    None => format!("plugin {id}: refreshed"),
                }),
                Err(why) => report.failed(match pinned {
                    Some(reference) => format!(
                        "plugin {id}: INSTALL AT {reference} FAILED twice ({why}); nothing was \
                         installed from tip and it is now MISSING until the next apply or run"
                    ),
                    None => format!(
                        "plugin {id}: REINSTALL FAILED twice ({why}); it is now MISSING until \
                         the next apply or run"
                    ),
                }),
            }
        }
        report
    }
}

#[cfg(test)]
mod tests;
