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

/// `[delivery]`: whatever is left of the table once the retry keys and the
/// remote deadline beside it have been read, which is nothing this schema
/// serves.
///
/// IT EXISTS TO REFUSE BY NAME. The retired `bypass_silence_classes` is the
/// key an operator is most likely still carrying, and a leftover key skipped
/// quietly is a setting they believe is in force.
pub(super) fn parse_delivery(value: toml::Value) -> Result<(), ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`delivery` is not a table".into()));
    };
    match table.keys().next() {
        None => Ok(()),
        Some(key) => {
            admits_flat("delivery", key)?;
            Err(unknown_key("delivery", "delivery", key))
        }
    }
}
