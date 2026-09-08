use crate::ConfigError;
use crate::config::schema::{absolute, admits_lane, non_empty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsConfig {
    pub lock: String,
    pub agents: String,
    pub claude_skills: String,
    pub hermes: String,
    pub npx: String,
    pub skills_cli_version: String,
    pub clawhub: String,
    pub hermes_cli: String,
    pub cua_driver: String,
    pub routing: String,
}
impl SkillsConfig {
    pub fn parse_fields(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        const KEYS: &[&str] = &[
            "type",
            "lock",
            "agents",
            "claude_skills",
            "hermes",
            "npx",
            "skills_cli_version",
            "clawhub",
            "hermes_cli",
            "cua_driver",
            "routing",
            "deadline_secs",
            "escalate_after_runs",
        ];
        for key in fields.keys() {
            admits_lane(label, "skills", KEYS, key)?;
        }
        let field = |key| {
            fields
                .get(key)
                .ok_or_else(|| ConfigError::Invalid(format!("`{label}` has no `{key}`")))
        };
        let path = |key| absolute(label, key, field(key)?);
        Ok(Self {
            lock: path("lock")?,
            agents: path("agents")?,
            claude_skills: path("claude_skills")?,
            hermes: path("hermes")?,
            npx: path("npx")?,
            skills_cli_version: non_empty(
                label,
                "skills_cli_version",
                field("skills_cli_version")?,
            )?,
            clawhub: path("clawhub")?,
            hermes_cli: path("hermes_cli")?,
            cua_driver: path("cua_driver")?,
            routing: path("routing")?,
        })
    }
}
#[cfg(test)]
mod tests;
