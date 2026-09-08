use crate::{BootstrapLane, CommandRunner, ConfigError, LaneAdapter, SkillsConfig};
use std::path::Path;
use uu_domain::{LaneReport, RunFacts};
impl LaneAdapter for SkillsConfig {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        // Configuration is composed before any lane can replace the running binary.
        let _ = super::capture_skills_updater();
        Self::parse_fields(label, fields)
    }
    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }
    fn run(&self, name: &str, _: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        match crate::home() {
            Some(home) => self.run_skills(name, Path::new(&home), runner),
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
    fn diagnostic_program(&self) -> Option<&str> {
        Some(&self.npx)
    }
}
impl BootstrapLane for SkillsConfig {
    fn bootstrap(&self, name: &str, home: &str, runner: &dyn CommandRunner) -> LaneReport {
        self.bootstrap_skills(name, Path::new(home), runner)
    }
}
