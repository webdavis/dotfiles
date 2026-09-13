use super::*;

pub(super) fn parse_phone(value: toml::Value) -> Result<Option<String>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`phone` is not a table".into()));
    };
    let mut marker = None;
    for (key, value) in table {
        admits_flat("phone", &key)?;
        let path = text("phone", &key, &value)?;
        if path.is_empty()
            || path.chars().any(char::is_control)
            || !(Path::new(&path).is_absolute() || path.starts_with("~/"))
        {
            return Err(ConfigError::Invalid(
                "`phone.marker_file` needs an absolute path or ~/ path without control characters"
                    .into(),
            ));
        }
        marker = Some(path);
    }
    Ok(marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_paths_are_explicit_and_values_never_leak_in_refusals() {
        for path in [
            "",
            "relative-secret",
            "~other/private",
            "/secret\npath",
            "/secret\0path",
        ] {
            let config = format!(
                "[phone]\nmarker_file = {}",
                serde_json::to_string(path).unwrap()
            );
            let error = parse_config(&config).unwrap_err();
            assert!(error.detail().contains("phone.marker_file"), "{error:?}");
            assert!(!error.detail().contains("secret"));
        }
        for path in ["/absolute/path", "~/attention"] {
            let config = format!("[phone]\nmarker_file = '{path}'");
            assert_eq!(
                parse_config(&config).unwrap().phone_marker_file.as_deref(),
                Some(path)
            );
        }
        assert!(parse_config("[phone]\nunknown = 'private'").is_err());
        assert!(parse_config("[phone]").unwrap().phone_marker_file.is_none());
    }
}
