use super::invoke;
use crate::config::NvimPluginsLane;
use crate::lanes::{CommandRunner, LaneAdapter};
use uu_domain::{LaneReport, RunFacts};

impl LaneAdapter for NvimPluginsLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_nvim_plugins_lane(label, fields)
    }
    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }
    fn diagnostic_program(&self) -> Option<&str> {
        Some(&self.host.nvim)
    }
    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        invoke(&self.host, "plugins", &[], name, runner)
    }
}
