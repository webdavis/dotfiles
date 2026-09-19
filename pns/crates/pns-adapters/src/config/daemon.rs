use super::github::job_interval;
use super::*;
use pns_application::PollSetting;

/// See `Config::daemon_enabled`.
pub(super) const DEFAULT_DAEMON_ENABLED: bool = true;

/// `[daemon]`'s one switch, in `parse_focus`'s shape: an unknown key inside
/// the table and a value of the wrong type are each refused BY NAME, rather
/// than half-read into a clock the operator believes they turned off.
pub(super) fn parse_daemon(value: toml::Value) -> Result<bool, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`daemon` is not a table".to_string()));
    };
    let mut enabled = DEFAULT_DAEMON_ENABLED;
    for (key, setting) in table {
        admits_flat("daemon", &key)?;
        match key.as_str() {
            "enabled" => {
                enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`daemon` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            _ => {
                return Err(unknown_key("daemon", "daemon", &key));
            }
        }
    }
    Ok(enabled)
}

pub struct DaemonConfig {
    pub home: String,
}
impl pns_application::DaemonSettings for DaemonConfig {
    fn enabled(&self) -> Result<bool, String> {
        match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => Ok(config.daemon_enabled),
            Ok(LoadOutcome::Missing) => Ok(true),
            Err(error) => Err(error.detail().to_string()),
        }
    }
    /// How often the GitHub poll runs, and `None` while the source is off,
    /// absent or refused.
    ///
    /// THE SERVER'S INTERVAL BEATS THE CONFIG'S, and the poll's own state file
    /// is where it is held: the documentation asks for `X-Poll-Interval` to be
    /// obeyed, so the config key is only ever the figure used before the first
    /// answer. A state file with no interval in it (a fresh machine, or one
    /// whose first poll has not answered yet) falls back to the key.
    fn github_interval(&self) -> PollSetting {
        let source = match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => parse_github(&config).ok().flatten(),
            Ok(LoadOutcome::Missing) => None,
            Err(_) => return PollSetting::Unreadable,
        };
        let Some(source) = source else {
            return PollSetting::Off;
        };
        let asked_for = crate::read_poll_state(&crate::state_dir()).interval_secs;
        PollSetting::Every(job_interval(source.poll_secs, asked_for))
    }
    /// How often the calendar poll runs, and `None` while the feature is off,
    /// names no command, or sits in a config that will not load.
    fn calendar_interval(&self) -> PollSetting {
        match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => setting(config.quiet_calendar.armed()),
            Ok(LoadOutcome::Missing) => PollSetting::Off,
            Err(_) => PollSetting::Unreadable,
        }
    }
    fn presence_interval(&self) -> PollSetting {
        match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => setting(
                parse_presence(&config)
                    .ok()
                    .flatten()
                    .map(|presence| presence.poll_secs),
            ),
            Ok(LoadOutcome::Missing) => PollSetting::Off,
            Err(_) => PollSetting::Unreadable,
        }
    }
}

/// A loaded config's answer: an interval is on, nothing is off.
fn setting(interval: Option<u64>) -> PollSetting {
    interval.map_or(PollSetting::Off, PollSetting::Every)
}
