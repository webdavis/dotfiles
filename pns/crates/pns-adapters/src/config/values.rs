use super::*;

/// One setting that has to be a string, refused BY NAME and BY TYPE.
pub(super) fn text(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
) -> Result<String, ConfigError> {
    stated.as_str().map(str::to_string).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{where_it_is}` key `{key}` has type `{}`, not a string",
            stated.type_str()
        ))
    })
}

/// One `[lights]` scalar, refused BY NAME outside its range.
///
/// BOTH ENDS, ALWAYS. A floor alone leaves a value that parses and cannot work;
/// a ceiling alone leaves the same at the other end. Each bound is argued at
/// the constant that holds it, and the refusal echoes both, so an operator
/// reading it learns the range rather than only that they missed it.
///
/// THE TABLE IS AN ARGUMENT because the behaviour tables are nested: an
/// operator told `lights` has no `high` would go looking in the wrong heading.
pub(super) fn bounded(
    table: &str,
    key: &str,
    setting: &toml::Value,
    low: u64,
    high: u64,
) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not a count between {low} and {high}",
            setting.type_str()
        )));
    };
    if count < low || count > high {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` is {count}, outside the {low} to {high} range"
        )));
    }
    Ok(count)
}

/// One `[recap]` switch. A value of any other type is refused BY NAME rather
/// than read as its own truthiness, which is what would leave a delivery on
/// while its config said otherwise.
pub(super) fn flag(key: &str, setting: &toml::Value) -> Result<bool, ConfigError> {
    setting.as_bool().ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`recap` key `{key}` has type `{}`, not boolean",
            setting.type_str()
        ))
    })
}

/// One key holding a list of plain strings, with the TABLE, the key and what
/// the list is FOR named in every refusal. The emptiness rules belong to the
/// callers, because what an empty list MEANS is theirs.
///
/// THE TABLE IS AN ARGUMENT because two tables now hold list keys, and a
/// refusal that named only the key would send an operator with both a `[recap]`
/// and a `[focus]` table looking in the wrong one.
pub(super) fn strings(
    table: &str,
    key: &str,
    noun: &str,
    setting: &toml::Value,
) -> Result<Vec<String>, ConfigError> {
    let Some(values) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not {noun}",
            setting.type_str()
        )));
    };
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`{table}` key `{key}` has a `{}` in it, not {noun}",
                    value.type_str()
                ))
            })
        })
        .collect()
}
