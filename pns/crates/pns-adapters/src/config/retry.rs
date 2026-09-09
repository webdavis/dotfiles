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
    // `retry_random_secs` is deliberately NOT accepted. The jitter it configured
    // is gone, and a key that parses but changes nothing is worse than one that
    // is refused: it reads as configured behavior. Leaving it out of this list
    // means the caller's unknown-key path names it, which is how every other
    // retired key in this file behaves.
    for key in ["retry_base_secs"] {
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
            "retry_base_secs" => backoff.base_secs = count,
            _ => return Err(unknown_key("delivery", "delivery", key)),
        }
    }
    Ok(backoff)
}
