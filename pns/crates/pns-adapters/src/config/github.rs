use super::*;

/// The `[plugins.github]` settings, typed.
///
/// `parse_presence`'S SHAPE AND FOR ITS REASONS: `Ok(None)` is the inert table
/// (absent, or present with the switch off), which is the reading every other
/// sensor's own table gives, and `Err` carries the reason so a report cannot
/// name `token` for a fault that was the interval.
///
/// THE TOKEN IS REQUIRED, refused by name rather than defaulted to empty,
/// because the notifications API answers 401 to an empty bearer and the poll
/// would report a configuration problem the config could have named itself.
pub fn parse_github(config: &Config) -> Result<Option<GithubSource>, ConfigError> {
    let Some(entry) = config.plugins.get(GITHUB).filter(|entry| entry.enabled) else {
        return Ok(None);
    };
    let settings = &entry.settings;
    let Some(token) = settings.get("token") else {
        return Err(ConfigError::Invalid(format!(
            "no `token` in [plugins.{GITHUB}]; it is the classic personal access token \
             with the `notifications` scope, which is the only token these endpoints take"
        )));
    };
    let token = text(GITHUB, "token", token)?;
    if token.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{GITHUB}` key `token` is empty; the notifications API answers 401 to that, \
             so the poll would report a configuration problem this file could name itself"
        )));
    }
    let poll_secs = match settings.get("poll_secs") {
        Some(setting) => bounded(GITHUB, "poll_secs", setting, MIN_POLL_SECS, MAX_POLL_SECS)?,
        None => DEFAULT_POLL_SECS,
    };
    Ok(Some(GithubSource { token, poll_secs }))
}

/// The GitHub notification source's settings.
pub struct GithubSource {
    /// The classic personal access token, `notifications` scope.
    pub token: String,
    /// How often the poll runs when the server has not asked for something
    /// else. The server's own `X-Poll-Interval` overrides it from the first
    /// answer onwards.
    pub poll_secs: u64,
}

/// The config-table name, spelled once: the registry roster, the settings
/// reader and the daemon's registration all select on it.
pub const GITHUB: &str = "github";

/// The starting interval, and the one the documentation states: "there is an
/// `X-Poll-Interval` header that specifies how often (in seconds) you are
/// allowed to poll ... Today it is 60."
///
/// IT IS ONLY EVER THE FIRST ONE. From the first answer the server's own
/// header decides, so a value here that disagreed with the header would last
/// exactly one tick.
pub const DEFAULT_POLL_SECS: u64 = 60;

/// The floor. SIXTY, because the documentation's own answer is 60 and a
/// smaller number is a request to be rate-limited: this is not a knob for
/// polling faster than the server allows, it is one for polling slower.
const MIN_POLL_SECS: u64 = 60;

/// The ceiling. ONE HOUR, past which a notification source is slower than
/// looking at the website, which is the thing it exists to replace.
const MAX_POLL_SECS: u64 = 3600;

#[cfg(test)]
#[path = "github/tests.rs"]
mod github_tests;
