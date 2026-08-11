//! The config edge: `~/.config/pns/config.toml` decides which plugins run.
//!
//! The file SELECTS; it never defines. Every plugin is compiled in, disabled
//! until its table says `enabled = true`, so a machine runs exactly what its
//! config names and nothing else. The settings inside a plugin's table are
//! free-form here: this layer proves the shape, the registry interprets the
//! contents, and neither knows the other's plugin names.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config, because a typo that turns every
//! notification off must not pass quietly; a MISSING file is its own honest
//! outcome, distinct from both error and emptiness, so the caller can say
//! "unconfigured" instead of guessing; unknown top-level keys are refused,
//! so `[plugin.hue]` cannot silently disable what `[plugins.hue]` enables.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One plugin's slice of the config: the selection flag, and its settings
/// with the flag itself removed, because `enabled` belongs to this layer and
/// everything else belongs to the plugin.
#[derive(Debug, PartialEq)]
pub struct PluginEntry {
    pub enabled: bool,
    pub settings: toml::Table,
}

/// The whole parsed file. Ordered, so listings and errors are deterministic.
#[derive(Debug, PartialEq, Default)]
pub struct Config {
    pub plugins: BTreeMap<String, PluginEntry>,
}

/// Why a config could not be used. Every variant carries the offender by
/// name, because "config invalid" without a noun is a hunt.
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// The file exists but is not TOML.
    Malformed(String),
    /// The TOML is well-formed but violates the schema.
    Invalid(String),
    /// The file exists and could not be read.
    Unreadable(String),
}

/// What loading found at the path. `Missing` is deliberately not an error:
/// an unconfigured machine is a state to report, not a fault to diagnose.
#[derive(Debug, PartialEq)]
pub enum LoadOutcome {
    Missing,
    Loaded(Config),
}

/// Where the config lives for a given home directory. Pure, so the path rule
/// is testable without an environment.
pub fn config_path(home: &str) -> PathBuf {
    let _ = home;
    todo!("R2b: home/.config/pns/config.toml")
}

/// The pure half: text in, config or a named refusal out.
pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    let _ = text;
    todo!("R2b: parse the TOML, validate the schema, split enabled from settings")
}

/// The IO edge: read the file at `path` and hand its text to the parser.
pub fn load_config(path: &Path) -> Result<LoadOutcome, ConfigError> {
    let _ = path;
    todo!("R2b: absent file is Missing; present file goes through parse_config")
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, LoadOutcome, config_path, load_config, parse_config};

    // --- path resolution ----------------------------------------------------

    #[test]
    fn the_config_lives_under_the_homes_dot_config_pns() {
        assert_eq!(
            config_path("/Users/operator"),
            std::path::PathBuf::from("/Users/operator/.config/pns/config.toml")
        );
    }

    // --- parsing and the schema ---------------------------------------------

    #[test]
    fn a_plugin_table_with_enabled_true_is_selected_and_keeps_its_settings() {
        let config = parse_config("[plugins.hue]\nenabled = true\nroom = \"office\"\n").unwrap();
        let hue = &config.plugins["hue"];
        assert!(hue.enabled);
        assert_eq!(
            hue.settings.get("room").and_then(|v| v.as_str()),
            Some("office")
        );
        assert!(
            !hue.settings.contains_key("enabled"),
            "the selection flag is this layer's, not a setting"
        );
    }

    #[test]
    fn an_absent_enabled_flag_reads_disabled_because_selection_is_explicit() {
        let config = parse_config("[plugins.hue]\nroom = \"office\"\n").unwrap();
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
    fn malformed_toml_is_a_loud_error_never_a_silent_empty_config() {
        // A config that fails to parse and quietly becomes "nothing enabled"
        // would turn every notification off with no trace.
        assert!(matches!(
            parse_config("not [ toml"),
            Err(ConfigError::Malformed(_))
        ));
    }

    // --- the IO edge --------------------------------------------------------

    #[test]
    fn a_missing_file_is_its_own_outcome_not_an_error_and_not_empty() {
        let outcome = load_config(std::path::Path::new("/nonexistent/pns-config-test.toml"));
        assert_eq!(outcome, Ok(LoadOutcome::Missing));
    }

    #[test]
    fn a_present_file_loads_through_the_parser() {
        let path = std::env::temp_dir().join(format!("pns-config-test-{}", std::process::id()));
        std::fs::write(&path, "[plugins.hue]\nenabled = true\n").unwrap();
        let outcome = load_config(&path);
        std::fs::remove_file(&path).ok();
        match outcome {
            Ok(LoadOutcome::Loaded(config)) => assert!(config.plugins["hue"].enabled),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn a_present_malformed_file_is_a_loud_error() {
        let path =
            std::env::temp_dir().join(format!("pns-config-malformed-{}", std::process::id()));
        std::fs::write(&path, "corrupt [").unwrap();
        let outcome = load_config(&path);
        std::fs::remove_file(&path).ok();
        assert!(matches!(outcome, Err(ConfigError::Malformed(_))));
    }
}
