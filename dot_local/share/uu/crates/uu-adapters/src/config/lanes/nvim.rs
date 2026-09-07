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
    Ok(NvimPluginsLane {
        host: NvimHost { nvim, config },
    })
}

impl NvimPluginsLane {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "config",
        "deadline_secs",
        "escalate_after_runs",
        "nvim",
        "type",
    ];
}

#[cfg(test)]
mod tests;
