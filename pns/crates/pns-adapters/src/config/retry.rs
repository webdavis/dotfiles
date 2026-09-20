use super::*;
use pns_domain::retry::RetryLimits;

pub(super) fn parse_retry(table: &mut toml::Table) -> Result<RetryLimits, ConfigError> {
    let mut limits = RetryLimits::default();
    if let Some(value) = table.remove("max_retries") {
        limits.max_retries = value
            .as_integer()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| {
                ConfigError::Invalid(
                    "`delivery` key `max_retries` must be a nonnegative integer".to_string(),
                )
            })?;
    }
    if let Some(setting) = table.remove("event_max_age") {
        limits.event_max_age_secs =
            duration_key("delivery", "event_max_age", &setting, event_max_age_range())?;
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
    if let Some(setting) = table.remove("retry_step") {
        backoff.step_secs = duration_key("delivery", "retry_step", &setting, retry_step_range())?;
    }
    Ok(backoff)
}

/// `event_max_age`, BOUNDED ON BOTH SIDES with zero carved out, as every other
/// duration key is.
///
/// THE FLOOR IS A MINUTE, an operator bound rather than a derived one: under a
/// minute is shorter than the shipped `retry_step`, so the default schedule
/// would never run even once.
///
/// THE CEILING IS THIRTY DAYS. Past that a queued leg outlives the machine
/// state that would make its page mean anything, and the shipped week sits
/// well inside it.
fn event_max_age_range() -> RangeInclusive<Duration> {
    Duration::from_secs(60)..=Duration::from_secs(30 * 24 * 3_600)
}

/// `retry_step`, on the same terms.
///
/// THE FLOOR IS A SECOND, which is the finest step the ledger's whole-second
/// due times can hold apart.
///
/// THE CEILING IS AN HOUR. The wait is this step times the retry count, so an
/// hour already puts the twentieth retry past the shipped `event_max_age`.
fn retry_step_range() -> RangeInclusive<Duration> {
    Duration::from_secs(1)..=Duration::from_secs(3_600)
}
