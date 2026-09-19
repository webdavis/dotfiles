use super::*;

/// `[delivery]`: whatever is left of the table once the retry keys beside it
/// have been read, which is nothing this schema serves.
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
