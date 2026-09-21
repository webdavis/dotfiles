use super::*;

/// `[paths]`: where this install keeps its state and where it looks for
/// channel executables.
///
/// ONE TABLE FOR BOTH, because they are the same question asked twice and
/// neither belongs to a plugin: an operator moving this install off the
/// default layout moves both in one place.
pub(super) fn parse_paths(value: toml::Value) -> Result<Paths, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`paths` is not a table".into()));
    };
    let mut paths = Paths::default();
    for (key, setting) in table {
        admits_flat("paths", &key)?;
        let written = directory("paths", &key, &setting)?;
        match key.as_str() {
            "state_dir" => paths.state_dir = Some(written),
            "channels_dir" => paths.channels_dir = Some(written),
            _ => return Err(unknown_key("paths", "paths", &key)),
        }
    }
    Ok(paths)
}

/// One directory an operator wrote, refused BY NAME when it is not one.
///
/// THE VALUE IS NEVER ECHOED, for `parse_phone`'s reason: a path can name a
/// person or a project, and a refusal is printed wherever the config is read.
fn directory(table: &str, key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    let written = text(table, key, setting)?;
    if written.is_empty()
        || written.chars().any(char::is_control)
        || !(Path::new(&written).is_absolute() || written.starts_with("~/"))
    {
        return Err(ConfigError::Invalid(format!(
            "`{table}.{key}` needs an absolute path or ~/ path without control characters"
        )));
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_key_is_read_into_its_own_field_and_a_bad_path_is_refused_without_echoing_it() {
        let paths =
            parse_config("[paths]\nstate_dir = '~/state'\nchannels_dir = '/opt/channels'\n")
                .unwrap()
                .paths;
        assert_eq!(paths.state_dir.as_deref(), Some("~/state"));
        assert_eq!(paths.channels_dir.as_deref(), Some("/opt/channels"));
        for written in ["", "relative-secret", "~other/secret", "/secret\npath"] {
            let text = format!(
                "[paths]\nstate_dir = {}\n",
                serde_json::to_string(written).unwrap()
            );
            let error = parse_config(&text).unwrap_err();
            assert!(error.detail().contains("paths.state_dir"), "{error:?}");
            assert!(!error.detail().contains("secret"), "{error:?}");
        }
        assert!(parse_config("[paths]\nstate = '/x'\n").is_err());
        assert_eq!(parse_config("[paths]\n").unwrap().paths, Paths::default());
    }
}
