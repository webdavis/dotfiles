use super::*;

/// `[delivery] remote_deadline`: how long one remote call may take, in
/// seconds. Removed from the table before the rest is walked, the way the
/// retry counts are.
pub(super) fn parse_remote_deadline(table: &mut toml::Table) -> Result<u64, ConfigError> {
    let Some(value) = table.remove("remote_deadline") else {
        return Ok(Config::default().remote_deadline_secs);
    };
    value
        .as_integer()
        .and_then(|seconds| u64::try_from(seconds).ok())
        .ok_or_else(|| {
            ConfigError::Invalid(
                "`delivery` key `remote_deadline` must be a nonnegative integer".into(),
            )
        })
}

pub(super) fn parse_delivery(value: toml::Value) -> Result<Vec<String>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`delivery` is not a table".into()));
    };
    let mut classes = Config::default().bypass_silence_classes;
    for (key, setting) in table {
        admits_flat("delivery", &key)?;
        match key.as_str() {
            "bypass_silence_classes" => classes = class_names(&setting)?,
            _ => return Err(unknown_key("delivery", "delivery", &key)),
        }
    }
    Ok(classes)
}

pub(super) fn class_names(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let classes = strings(
        "delivery",
        "bypass_silence_classes",
        "a list of class names",
        setting,
    )?;
    if classes
        .iter()
        .any(|name| pns_protocol::Name::new(name).is_err())
    {
        return Err(ConfigError::Invalid(
            "`delivery` key `bypass_silence_classes` requires nonempty names of at most 64 characters without controls".into(),
        ));
    }
    Ok(classes)
}
