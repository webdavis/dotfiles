use super::github::job_interval;
use super::*;

/// See `Config::gateway_enabled`.
pub(super) const DEFAULT_GATEWAY_ENABLED: bool = true;

/// `[gateway]`'s two keys: the clock switch, and the launchd label `pns
/// gateway` acts on. IN `parse_remind`'s SHAPE, a named struct rather than a
/// pair of same-typed values, and for the same reason: an unknown key inside
/// the table and a value of the wrong type are each refused BY NAME, rather
/// than half-read into a clock or a service label the operator believes they set.
pub(super) struct GatewayTable {
    pub enabled: bool,
    pub service: Option<String>,
}

pub(super) fn parse_gateway(value: toml::Value) -> Result<GatewayTable, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`gateway` is not a table".to_string()));
    };
    let mut gateway = GatewayTable {
        enabled: DEFAULT_GATEWAY_ENABLED,
        service: None,
    };
    for (key, setting) in table {
        admits_flat("gateway", &key)?;
        match key.as_str() {
            "enabled" => {
                gateway.enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`gateway` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "service" => {
                gateway.service = Some(text("gateway", "service", &setting)?);
            }
            _ => {
                return Err(unknown_key("gateway", "gateway", &key));
            }
        }
    }
    Ok(gateway)
}

pub struct DaemonConfig {
    pub home: String,
}
impl pns_application::DaemonSettings for DaemonConfig {
    fn enabled(&self) -> Result<bool, String> {
        match load_config(&config_path(&self.home)) {
            Ok(LoadOutcome::Loaded(config)) => Ok(config.gateway_enabled),
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
                .map(|presence| presence.poll_interval_secs),
            _ => None,
        }
    }
}
