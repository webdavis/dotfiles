use crate::config::ConfigError;
use crate::config::schema::{absolute, admits_lane};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotateLogsLane {
    pub(crate) logs: Vec<String>,
    pub(crate) rotate_at_bytes: u64,
    pub(crate) archives_kept: u64,
    pub(crate) compressor: String,
}

impl RotateLogsLane {
    pub(crate) const KEYS: &'static [&'static str] = &[
        "type",
        "logs",
        "rotate_at_bytes",
        "archives_kept",
        "compressor",
        "deadline_secs",
        "escalate_after_runs",
    ];

    pub(crate) fn parse_fields(label: &str, fields: toml::Table) -> Result<Self, ConfigError> {
        for key in fields.keys() {
            admits_lane(label, "rotate-logs", Self::KEYS, key)?;
        }
        let stated = required(label, &fields, "logs")?;
        let logs = stated
            .as_array()
            .filter(|logs| !logs.is_empty())
            .ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`{label}` key `logs` must be a non-empty list of absolute paths"
                ))
            })?;
        let logs = logs
            .iter()
            .map(|path| absolute(label, "logs", path))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            logs,
            rotate_at_bytes: positive(label, &fields, "rotate_at_bytes")?,
            archives_kept: positive(label, &fields, "archives_kept")?,
            compressor: absolute(label, "compressor", required(label, &fields, "compressor")?)?,
        })
    }
}

fn required<'a>(
    label: &str,
    fields: &'a toml::Table,
    key: &str,
) -> Result<&'a toml::Value, ConfigError> {
    fields
        .get(key)
        .ok_or_else(|| ConfigError::Invalid(format!("`{label}` has no `{key}`")))
}

fn positive(label: &str, fields: &toml::Table, key: &str) -> Result<u64, ConfigError> {
    required(label, fields, key)?
        .as_integer()
        .filter(|value| *value >= 1)
        .map(|value| value as u64)
        .ok_or_else(|| {
            ConfigError::Invalid(format!(
                "`{label}` key `{key}` must be an integer at least 1"
            ))
        })
}

#[cfg(test)]
mod tests;
