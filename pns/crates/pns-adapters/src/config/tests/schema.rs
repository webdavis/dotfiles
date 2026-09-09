use super::*;

// --- parsing and the schema ---------------------------------------------

#[test]
fn a_plugin_table_with_enabled_true_is_selected_and_keeps_its_settings() {
    let config = parse_config("[plugins.hue]\nenabled = true\nbridge = \"office\"\n").unwrap();
    let hue = &config.plugins["hue"];
    assert!(hue.enabled);
    assert_eq!(
        hue.settings.get("bridge").and_then(|v| v.as_str()),
        Some("office")
    );
    assert!(
        !hue.settings.contains_key("enabled"),
        "the selection flag is this layer's, not a setting"
    );
}

#[test]
fn an_absent_enabled_flag_reads_disabled_because_selection_is_explicit() {
    let config = parse_config("[plugins.hue]\nbridge = \"office\"\n").unwrap();
    assert!(!config.plugins["hue"].enabled);
}

#[test]
fn an_empty_config_is_valid_and_selects_nothing() {
    let config = parse_config("").unwrap();
    assert!(config.plugins.is_empty());
}

#[test]
fn a_non_boolean_enabled_flag_is_refused_naming_the_plugin() {
    let err = parse_config("[plugins.hue]\nenabled = \"yes\"\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(message.contains("hue"), "the offender is named: {message}")
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn an_unknown_top_level_key_is_refused_so_a_typo_cannot_disable_a_channel() {
    // [plugin.hue] instead of [plugins.hue] must be a loud refusal, never
    // a quietly ignored table that leaves hue disabled.
    let err = parse_config("[plugin.hue]\nenabled = true\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("plugin"),
                "the offender is named: {message}"
            )
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_plugin_entry_that_is_not_a_table_is_refused_naming_the_plugin() {
    let err = parse_config("[plugins]\nhue = true\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(message.contains("hue"), "the offender is named: {message}")
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_stale_top_level_home_table_is_refused_by_name_rather_than_ignored() {
    // The probe's settings moved into `[plugins.router]`. A config still
    // carrying `[home]` must be refused NAMING it, so the operator is sent
    // to the one table they have to move; admitting it as a key nothing
    // reads any more would leave `pns home` reporting "not configured"
    // beside a file that plainly configures it.
    let err = parse_config("[home]\nrouter_url = \"https://192.168.1.1\"\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(message.contains("home"), "the offender is named: {message}")
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_non_table_plugins_value_is_refused_naming_the_key() {
    // `plugins = 5` at the one key the whole file hangs off must refuse,
    // never parse to an empty config with everything silently disabled.
    let err = parse_config("plugins = 5\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("plugins"),
                "the offender is named: {message}"
            )
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_malformed_line_is_reported_without_echoing_its_value() {
    // The config carries plugin secrets, and error strings travel to
    // logs: the refusal names where and why, never the line's contents.
    let err = parse_config("[plugins.mobile]\ntoken = \"SUPERSECRET\" trailing\n").unwrap_err();
    match err {
        ConfigError::Malformed(message) => {
            assert!(!message.is_empty(), "the cause is still named");
            assert!(
                !message.contains("SUPERSECRET"),
                "the offending line's value must not be echoed: {message}"
            );
        }
        other => panic!("expected Malformed, got {other:?}"),
    }
}

#[test]
fn malformed_toml_is_a_loud_error_never_a_silent_empty_config() {
    // A config that fails to parse and quietly becomes "nothing enabled"
    // would turn every notification off with no trace.
    assert!(matches!(
        parse_config("not [ toml"),
        Err(ConfigError::Malformed(_))
    ));
}
