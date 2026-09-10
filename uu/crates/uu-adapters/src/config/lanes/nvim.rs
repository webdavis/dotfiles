use crate::config::ConfigError;
use crate::config::schema::{absolute, admits_lane, boolean, non_empty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NvimHost {
    pub(crate) nvim: String,
    pub(crate) config: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvimPluginsLane {
    pub(crate) host: NvimHost,
    pub(crate) auto_commit: bool,
    pub(crate) repo: Option<String>,
}

pub(crate) fn parse_nvim_plugins_lane(
    label: &str,
    fields: toml::Table,
) -> Result<NvimPluginsLane, ConfigError> {
    for name in fields.keys() {
        admits_lane(label, "nvim-plugins", NvimPluginsLane::KEYS, name)?;
    }
    let host = host(label, &fields)?;
    let auto_commit = fields
        .get("auto_commit")
        .map(|v| boolean(label, "auto_commit", v))
        .transpose()?
        .unwrap_or(false);
    let repo = fields
        .get("repo")
        .map(|v| absolute(label, "repo", v))
        .transpose()?;
    if auto_commit && repo.is_none() {
        return Err(ConfigError::Invalid(format!(
            "`{label}` has `auto_commit` enabled but no `repo`; state its absolute source repository path"
        )));
    }
    Ok(NvimPluginsLane {
        host,
        auto_commit,
        repo,
    })
}

impl NvimPluginsLane {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "auto_commit",
        "config",
        "deadline_secs",
        "escalate_after_runs",
        "nvim",
        "repo",
        "type",
    ];
}

fn host(label: &str, fields: &toml::Table) -> Result<NvimHost, ConfigError> {
    let nvim = fields
        .get("nvim")
        .map(|v| non_empty(label, "nvim", v))
        .transpose()?
        .unwrap_or_else(|| "nvim".into());
    let config = fields.get("config").ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{label}` has no `config`; state the absolute Neovim config directory"
        ))
    })?;
    let config = absolute(label, "config", config)?;
    Ok(NvimHost { nvim, config })
}

impl NvimHost {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "config",
        "deadline_secs",
        "escalate_after_runs",
        "nvim",
        "type",
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvimMasonLane {
    pub(crate) host: NvimHost,
}

pub(crate) fn parse_nvim_mason_lane(
    label: &str,
    fields: toml::Table,
) -> Result<NvimMasonLane, ConfigError> {
    for key in fields.keys() {
        admits_lane(label, "nvim-mason", NvimHost::KEYS, key)?;
    }
    Ok(NvimMasonLane {
        host: host(label, &fields)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvimParsersLane {
    pub(crate) host: NvimHost,
}

pub(crate) fn parse_nvim_parsers_lane(
    label: &str,
    fields: toml::Table,
) -> Result<NvimParsersLane, ConfigError> {
    for key in fields.keys() {
        admits_lane(label, "nvim-parsers", NvimHost::KEYS, key)?;
    }
    Ok(NvimParsersLane {
        host: host(label, &fields)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvimSmokeTestLane {
    pub(crate) host: NvimHost,
    pub(crate) cache: String,
}

impl NvimSmokeTestLane {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "cache",
        "config",
        "deadline_secs",
        "escalate_after_runs",
        "nvim",
        "type",
    ];
    pub(crate) fn parse(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        for key in fields.keys() {
            admits_lane(label, "nvim-smoke-test", Self::KEYS, key)?;
        }
        let cache = fields.get("cache").ok_or_else(|| {
            ConfigError::Invalid(format!(
                "`{label}` has no `cache`; state the absolute candidate tree directory"
            ))
        })?;
        Ok(Self {
            host: host(label, &fields)?,
            cache: absolute(label, "cache", cache)?,
        })
    }
}

#[cfg(test)]
mod tests;
