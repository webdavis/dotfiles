use super::*;

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
