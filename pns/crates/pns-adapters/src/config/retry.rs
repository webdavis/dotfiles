use super::*;
use pns_domain::retry::RetryLimits;

pub(super) fn parse_retry(table: &mut toml::Table) -> Result<RetryLimits, ConfigError> {
    let mut limits = RetryLimits::default();
    for key in ["max_attempts", "max_age_secs"] {
        let Some(value) = table.remove(key) else {
            continue;
        };
        let count = value
            .as_integer()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`delivery` key `{key}` must be a nonnegative integer"
                ))
            })?;
        match key {
            "max_attempts" => limits.max_attempts = count,
            "max_age_secs" => limits.max_age_secs = count,
            _ => return Err(unknown_key("delivery", "delivery", key)),
        }
    }
    Ok(limits)
}

pub(super) fn parse_backoff(
    table: &mut toml::Table,
) -> Result<pns_domain::retry::RetryBackoff, ConfigError> {
    let mut backoff = pns_domain::retry::RetryBackoff::default();
    for key in ["retry_base_secs", "retry_random_secs"] {
        let Some(value) = table.remove(key) else {
            continue;
        };
        let count = value
            .as_integer()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`delivery` key `{key}` must be a nonnegative integer"
                ))
            })?;
        match key {
            "retry_base_secs" => {
                backoff.base_secs = count;
                backoff.random_secs = count;
            }
            "retry_random_secs" => backoff.random_secs = count,
            _ => return Err(unknown_key("delivery", "delivery", key)),
        }
    }
    Ok(backoff)
}
