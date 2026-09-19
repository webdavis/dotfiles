use super::github::job_interval;
use super::*;

/// See `Config::daemon_enabled`.
pub(super) const DEFAULT_DAEMON_ENABLED: bool = true;

/// `[daemon]`'s two keys: the clock switch, and the launchd label `pns
/// gateway` acts on. IN `parse_nag`'s SHAPE, a named struct rather than a
/// pair of same-typed values, and for the same reason: an unknown key inside
/// the table and a value of the wrong type are each refused BY NAME, rather
/// than half-read into a clock or a gateway the operator believes they set.
pub(super) struct DaemonTable {
    pub enabled: bool,
    pub service: Option<String>,
}

pub(super) fn parse_daemon(value: toml::Value) -> Result<DaemonTable, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`daemon` is not a table".to_string()));
    };
    let mut daemon = DaemonTable {
        enabled: DEFAULT_DAEMON_ENABLED,
        service: None,
    };
    for (key, setting) in table {
        admits_flat("daemon", &key)?;
        match key.as_str() {
            "enabled" => {
                daemon.enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`daemon` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "service" => {
                daemon.service = Some(text("daemon", "service", &setting)?);
            }
            _ => {
                return Err(unknown_key("daemon", "daemon", &key));
            }
        }
    }
    Ok(daemon)
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
    fn github_interval(&self) -> Option<u64> {
        let source = match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => parse_github(&config).ok().flatten(),
            _ => None,
        }?;
        let asked_for = crate::read_poll_state(&crate::state_dir()).interval_secs;
        Some(job_interval(source.poll_secs, asked_for))
    }
    /// How often the calendar poll runs, and `None` while the feature is off,
    /// names no command, or sits in a config that will not load.
    fn calendar_interval(&self) -> Option<u64> {
        match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => config.quiet_calendar.armed(),
            _ => None,
        }
    }
    fn presence_interval(&self) -> Option<u64> {
        match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => parse_presence(&config)
                .ok()
                .flatten()
                .map(|presence| presence.poll_secs),
            _ => None,
        }
    }
}
