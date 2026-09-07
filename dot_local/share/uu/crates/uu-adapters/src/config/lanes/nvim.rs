use crate::config::ConfigError;
use crate::config::schema::{absolute, admits_lane, non_empty};

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
    let auto_commit = fields
        .get("auto_commit")
        .map(|v| {
            v.as_bool().ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`{label}` key `auto_commit` must be true or false, got {v:?}"
                ))
            })
        })
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
        host: NvimHost { nvim, config },
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

#[cfg(test)]
mod tests;
