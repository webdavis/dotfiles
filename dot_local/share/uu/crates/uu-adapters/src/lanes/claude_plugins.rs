mod inventory;
mod snapshot;
use crate::config::ClaudePluginsLane;
use crate::{BootstrapLane, CommandRunner, ConfigError, LaneAdapter};
use uu_domain::{LaneReport, RunFacts};
impl LaneAdapter for ClaudePluginsLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        Self::parse_fields(label, fields)
    }
    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }
    fn run(&self, name: &str, _: &RunFacts, _: &dyn CommandRunner) -> LaneReport {
        match crate::home() {
            Some(home) => execute(self, name, &home, false),
            None => {
                let mut report = LaneReport::new(name);
                report.failed("HOME is not set".into());
                report
            }
        }
    }
    fn bootstrap_capability(&self) -> Option<&dyn BootstrapLane> {
        Some(self)
    }
}
impl BootstrapLane for ClaudePluginsLane {
    fn bootstrap(&self, name: &str, home: &str, _: &dyn CommandRunner) -> LaneReport {
        execute(self, name, home, true)
    }
}
const LABEL: &str = "Claude Code plugins";
const CAVEAT: &str = "Versions are what Claude Code records in installed_plugins.json for USER-scope installs; a project-scope plugin is not tracked here. A plugin whose marketplace publishes neither a version nor a commit reports the literal `unknown`, so its updates cannot appear in this list at all.";

fn execute(lane: &ClaudePluginsLane, name: &str, home: &str, bootstrap: bool) -> LaneReport {
    let mut report = LaneReport::new(name);
    match update(lane, name, home, bootstrap) {
        Ok(line) => report.noted(line),
        Err(error) => report.failed(format!(
            "{LABEL}: NOT COMPARED, {error}; snapshot was not advanced"
        )),
    }
    report
}

fn update(
    lane: &ClaudePluginsLane,
    name: &str,
    home: &str,
    bootstrap: bool,
) -> Result<String, String> {
    let before = snapshot::baseline(home, name)?;
    if bootstrap && before.is_some() {
        return Ok(format!("{LABEL}: existing baseline kept; no comparison"));
    }
    let text = snapshot::read(std::path::Path::new(&lane.inventory))?
        .ok_or_else(|| format!("inventory {} is absent", lane.inventory))?;
    let after = inventory::read_inventory(&text)?;
    let line = match before {
        Some(before) => super::changes::section::change_section(
            &Ok(before),
            &Ok(after.clone()),
            LABEL,
            CAVEAT,
            "reading ~/.claude/plugins/installed_plugins.json",
        ),
        None => {
            format!("{LABEL}: first reading saved as the baseline; nothing was compared. {CAVEAT}")
        }
    };
    // Unlike the retired bash job, this lane advances before the combined run record is delivered.
    // The approved lane contract reports a failed publication and does not silently reseed it.
    snapshot::advance(home, name, &after)?;
    Ok(line)
}
#[cfg(test)]
mod tests;
