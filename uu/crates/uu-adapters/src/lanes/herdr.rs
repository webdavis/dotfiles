//! The herdr lane: the binary refreshes itself, then every UNPINNED plugin in
//! the roster is reinstalled at its source's tip while every pinned one is
//! left where it is and reported on.
//!
//! herdr HAS NO `plugin update`, so a refresh is an uninstall followed by a
//! fresh install. An entry with no `ref` re-pins at the source's tip, which is
//! what every entry has always done.
//!
//! A PINNED PLUGIN IS NEVER TOUCHED BY THE WEEKLY RUN. `herdr plugin list
//! --json` says which revision each installed plugin sits at: an entry already
//! at its `ref` is reported HELD, and one that is not (a pin the operator has
//! moved, or a plugin nothing has installed yet) is reported PENDING with the
//! exact `herdr plugin install --ref` command, the same way the cargo lane
//! reports a crate it will not compile. An unattended run that reinstalled a
//! pin would have to uninstall first, so a revision that stopped resolving
//! would take the working copy with it; the operator runs that install with
//! the failure in front of them instead.
//!
//! WITHOUT THAT LISTING A PIN IS NOT CHECKED. Every pinned entry is left alone
//! and the step is failed by name, because reporting HELD on an answer nobody
//! read is the one outcome that makes a pin stop being a decision.
//!
//! A failed install RETRIES ONCE and a plugin that still failed is named
//! loudly, so the record for the week says exactly what is missing. The
//! running server keeps its loaded plugins until restart, so a failure costs
//! the next restart rather than the current session.
//!
//! THE RUN EVENT IS UNUSED HERE: this lane predates it and drives herdr by
//! argv alone.

mod listing;

use std::collections::BTreeMap;

use crate::config::{HerdrLane, Plugin};
use crate::lanes::{CommandRunner, LaneAdapter};
use listing::{Installed, parse_plugin_list, pin_sentence};
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
        self.update_itself(runner, &mut report);
        // ONE LISTING FOR THE WHOLE LANE, and none at all when nothing is
        // pinned: an unpinned roster is reinstalled either way, so asking
        // herdr where its plugins sit would answer a question nobody asked.
        let pinned: Vec<&Plugin> = self
            .plugins
            .iter()
            .filter(|entry| entry.pinned_ref.is_some())
            .collect();
        // Read once and match once: a listing failure is ONE failure for the
        // whole lane, not one per pinned entry re-hitting the same Err.
        let installed = if pinned.is_empty() {
            Some(BTreeMap::new())
        } else {
            match self.installed(runner) {
                Ok(installed) => Some(installed),
                Err(why) => {
                    let names = pinned
                        .iter()
                        .map(|p| {
                            format!(
                                "plugin {} ({})",
                                p.id,
                                p.pinned_ref.as_deref().unwrap_or("")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    report.failed(format!(
                        "{why}; every pinned plugin LEFT ALONE: {names}, so whether they sit at \
                         their revisions is unknown"
                    ));
                    None
                }
            }
        };
        for plugin in &self.plugins {
            match (plugin.pinned_ref.as_deref(), &installed) {
                (Some(pin), Some(installed)) => self.hold(plugin, pin, installed, &mut report),
                (Some(_), None) => {}
                (None, _) => self.refresh(plugin, runner, &mut report),
            }
        }
        report
    }
}

impl HerdrLane {
    fn update_itself(&self, runner: &dyn CommandRunner, report: &mut LaneReport) {
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
    }

    fn installed(&self, runner: &dyn CommandRunner) -> Result<BTreeMap<String, Installed>, String> {
        runner
            .run(&self.binary, &["plugin", "list", "--json"])
            .map_err(|why| format!("`plugin list --json` could not be read ({why})"))
            .and_then(|stdout| parse_plugin_list(&stdout))
    }

    /// A pinned plugin: checked against the listing, never installed here.
    fn hold(
        &self,
        plugin: &Plugin,
        pin: &str,
        installed: &BTreeMap<String, Installed>,
        report: &mut LaneReport,
    ) {
        let id = plugin.id.as_str();
        let at = installed.get(id);
        match at {
            Some(at) if at.holds(pin) => {
                report.noted(format!("plugin {id}: HELD at {pin} (pinned, not updated)"));
            }
            at => report.pending(pin_sentence(&self.binary, id, &plugin.repo, pin, at)),
        }
    }

    /// An unpinned plugin: uninstalled, then reinstalled at its source's tip.
    fn refresh(&self, plugin: &Plugin, runner: &dyn CommandRunner, report: &mut LaneReport) {
        let id = plugin.id.as_str();
        // AN INSTALL OVER A FAILED UNINSTALL IS NOT ATTEMPTED. herdr pins
        // a plugin at install, so installing on top of a copy that would
        // not come off is how one plugin becomes two.
        if let Err(why) = runner.run(&self.binary, &["plugin", "uninstall", id]) {
            report.failed(format!(
                "plugin {id}: uninstall failed ({why}); leaving the installed copy alone"
            ));
            return;
        }
        let args = ["plugin", "install", plugin.repo.as_str(), "--yes"];
        let install = || runner.run(&self.binary, &args);
        // The retry is the SECOND call and there is no third: `or_else`
        // runs it only on a failure, and the reason kept is the one the
        // last attempt gave.
        match install().or_else(|_| install()) {
            Ok(_) => report.noted(format!("plugin {id}: refreshed")),
            Err(why) => report.failed(format!(
                "plugin {id}: REINSTALL FAILED twice ({why}); it is now MISSING until the next \
                 apply or run"
            )),
        }
    }
}

#[cfg(test)]
mod tests;
