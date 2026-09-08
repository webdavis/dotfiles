use crate::config::ConfigError;
use crate::config::schema::{absolute, admits_lane};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudePluginsLane {
    pub(crate) inventory: String,
}

impl ClaudePluginsLane {
    pub(crate) const KEYS: &'static [&'static str] =
        &["type", "inventory", "deadline_secs", "escalate_after_runs"];
    pub(crate) fn parse_fields(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        let mut inventory = None;
        for (key, value) in fields {
            admits_lane(label, "claude-plugins", Self::KEYS, &key)?;
            if key == "inventory" {
                inventory = Some(absolute(label, &key, &value)?);
            }
        }
        let inventory = inventory.ok_or_else(|| {
            ConfigError::Invalid(format!(
                "`{label}` has no `inventory`; state its absolute path"
            ))
        })?;
        Ok(Self { inventory })
    }
}
#[cfg(test)]
mod tests;
