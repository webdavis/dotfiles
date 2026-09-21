use super::*;

/// `[producer.<name>]`: what the operator asked for per producer, keyed by the
/// name that producer sends.
///
/// ONE TABLE PER PRODUCER, SINGULAR, which is `[lights.lamp."<name>"]`'s own
/// shape: the heading names one producer rather than a set of them.
///
/// THE NAMES ARE THE OPERATOR'S, so this level refuses nothing by name; a
/// producer nothing sends under is an entry no event ever resolves against,
/// the way a project with no channel is. What IS refused is an entry that is
/// not a table and a key inside one that this schema does not serve.
pub(super) fn parse_producer(value: toml::Value) -> Result<BTreeMap<String, bool>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`producer` is not a table".to_string(),
        ));
    };
    let mut remind = BTreeMap::new();
    for (name, entry) in table {
        let toml::Value::Table(settings) = entry else {
            return Err(ConfigError::Invalid(format!(
                "`producer` key `{name}` has type `{}`, not a table like [producer.{name}]",
                entry.type_str()
            )));
        };
        let shown = format!("producer.{name}");
        for (key, setting) in settings {
            admits(PRODUCER_KEYS, &shown, &key)?;
            match key.as_str() {
                "remind" => {
                    let Some(armed) = setting.as_bool() else {
                        return Err(ConfigError::Invalid(format!(
                            "`{shown}` key `remind` has type `{}`, not boolean",
                            setting.type_str()
                        )));
                    };
                    remind.insert(name.clone(), armed);
                }
                _ => return Err(unknown_key(PRODUCER_KEYS, &shown, &key)),
            }
        }
    }
    Ok(remind)
}
