use super::*;

/// `[failures]`: whether the failure record is served as a page, and where.
///
/// TWO DEFAULTED KEYS, not an opt-in feature, which is why both ship uncommented
/// at their defaults. The page is an extra rung on a ladder that already stands
/// without it: the notification is self-sufficient, Discord carries the full
/// form whenever the hermes leg worked, and `pns failures` is unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failures {
    /// Whether the daemon serves the page at all.
    ///
    /// DEFAULT ON, and `false` is the fallback for anyone whose phone cannot
    /// reach it. pns cannot detect a moshi Pro subscription and does not need
    /// to: an operator who cannot use browser preview writes `false`, and that
    /// is the same switch that decides which pointer the phone's fix line gets.
    pub serve: bool,
    /// The loopback port the page listens on.
    ///
    /// FIXED AND DOCUMENTED so discovery is predictable: `moshi-hook` probes
    /// local listeners and remembers the ones answering with an HTTP header, so
    /// nothing registers itself, and an operator with a narrowed `scan-ports`
    /// list needs to know which number to add.
    pub port: u16,
}

/// See [`Failures::serve`].
const DEFAULT_FAILURES_SERVE: bool = true;

/// See [`Failures::port`].
const DEFAULT_FAILURES_PORT: u16 = 8646;

/// The lowest port this will bind. Everything below 1024 needs root on macOS,
/// and the daemon runs as the operator, so a privileged number is a config that
/// cannot do what it says rather than a preference.
const MIN_FAILURES_PORT: i64 = 1024;

impl Default for Failures {
    fn default() -> Self {
        Failures {
            serve: DEFAULT_FAILURES_SERVE,
            port: DEFAULT_FAILURES_PORT,
        }
    }
}

/// `[failures]`, in `parse_nag`'s shape: an unknown key and a value of the wrong
/// shape are each refused BY NAME rather than half-read into a page the operator
/// believes they configured.
pub(super) fn parse_failures(value: toml::Value) -> Result<Failures, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`failures` is not a table".to_string(),
        ));
    };
    let mut failures = Failures::default();
    for (key, setting) in table {
        admits_flat("failures", &key)?;
        match key.as_str() {
            "serve" => {
                failures.serve = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`failures` key `serve` has type `{}`, not a true or false",
                        setting.type_str()
                    ))
                })?
            }
            "port" => failures.port = port(&setting)?,
            _ => return Err(unknown_key("failures", "failures", &key)),
        }
    }
    Ok(failures)
}

/// The port, REFUSED RATHER THAN CLAMPED in `nag_schedule`'s style: a silently
/// corrected port is a port the operator believes they set, and they would go
/// looking for the page on the number they wrote.
fn port(setting: &toml::Value) -> Result<u16, ConfigError> {
    let Some(number) = setting.as_integer() else {
        return Err(ConfigError::Invalid(format!(
            "`failures` key `port` has type `{}`, not a port number",
            setting.type_str()
        )));
    };
    if !(MIN_FAILURES_PORT..=i64::from(u16::MAX)).contains(&number) {
        return Err(ConfigError::Invalid(format!(
            "`failures` key `port` is {number}, outside the {MIN_FAILURES_PORT} to {} \
             range this can bind",
            u16::MAX
        )));
    }
    Ok(number as u16)
}

#[cfg(test)]
#[path = "failures/tests.rs"]
mod tests;
