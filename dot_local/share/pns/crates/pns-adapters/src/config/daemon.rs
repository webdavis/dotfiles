use super::*;

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
