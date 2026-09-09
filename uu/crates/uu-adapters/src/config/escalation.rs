use super::ConfigError;
use std::num::NonZeroU32;

pub(super) fn parse_escalation(
    table: &str,
    value: &toml::Value,
) -> Result<NonZeroU32, ConfigError> {
    value.as_integer().and_then(|number| u32::try_from(number).ok()).and_then(NonZeroU32::new)
        .ok_or_else(|| ConfigError::Invalid(format!(
            "`{table}` key `escalate_after_runs` must be a positive whole number no greater than {}", u32::MAX
        )))
}

#[cfg(test)]
mod tests;
