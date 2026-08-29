//! The config edge: `~/.config/pns/config.toml` decides which plugins run.
//!
//! The file SELECTS; it never defines. Every plugin is compiled in, disabled
//! until its table says `enabled = true`, so a machine runs exactly what its
//! config names and nothing else. The settings inside a plugin's table are
//! free-form here: this layer proves the shape, the registry interprets the
//! contents, and neither knows the other's plugin names.
//!
//! `[recap]` is the one top-level table that is not a plugin, and the second
//! key admitted here: three booleans and a count THIS layer reads itself.
//! Because it reads them, it can judge them, so an unknown key inside it, and a
//! count that is not a threshold, are refused rather than passed along the way
//! a plugin's settings are.
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

/// The recap's three independent delivery switches.
///
/// ABSENT IS ALL ON, which is what makes the table optional: a machine that
/// never writes one behaves exactly as it did before the table existed. Each
/// boolean gates ONLY its own delivery, so recap-only and card-only are both
/// valid configurations and neither implies the other.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived. `#[derive(Default)]` reads
/// a bool as false, which would take every delivery away from every machine
/// whose config was written before this table existed, and it would do it
/// silently.
///
/// `min_events` IS THE ONE COUNT HERE, and it is a key rather than a constant
/// because nobody can calibrate it yet: the locked volume threshold carries a
/// tilde, and the machine it was written for has no history to measure. The
/// recap prints the window's real count in its own header every time, so one
/// week of real recaps settles the number without a rebuild.
///
/// COPY, so the composition root can read it off a borrowed config and carry
/// it as ONE value rather than as a row of loose booleans. Four adjacent bools
/// in one call are a swap waiting to happen; named fields are not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recap {
    pub replay_card: bool,
    pub digest: bool,
    pub digest_as_thread: bool,
    pub min_events: usize,
}

impl Default for Recap {
    fn default() -> Self {
        Recap {
            replay_card: true,
            digest: true,
            digest_as_thread: true,
            min_events: DEFAULT_MIN_EVENTS,
        }
    }
}

/// How many events a window needs before a recap is worth the operator's
/// attention. The operator's own stated figure; see `Recap`.
const DEFAULT_MIN_EVENTS: usize = 8;

/// The whole parsed file. Ordered, so listings and errors are deterministic.
#[derive(Debug, PartialEq, Default)]
pub struct Config {
    pub plugins: BTreeMap<String, PluginEntry>,
    pub recap: Recap,
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

impl ConfigError {
    /// What went wrong, already sanitized for printing. Each mode wraps it in
    /// the sentence describing what IT did about it.
    pub fn detail(&self) -> &str {
        match self {
            ConfigError::Malformed(detail)
            | ConfigError::Invalid(detail)
            | ConfigError::Unreadable(detail) => detail,
        }
    }
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
    Path::new(home).join(".config/pns/config.toml")
}

/// The pure half: text in, config or a named refusal out.
pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    // The parser's Display echoes the offending source line, and this file
    // carries plugin secrets into log lines, so the refusal is rebuilt from
    // the cause and the location alone.
    let document: toml::Table = text.parse().map_err(|error: toml::de::Error| {
        let line = error
            .span()
            .map(|span| text[..span.start].matches('\n').count() + 1);
        ConfigError::Malformed(match line {
            Some(line) => format!("{} at line {line}", error.message()),
            None => error.message().to_string(),
        })
    })?;

    let mut config = Config::default();
    // TWO ADMITTED KEYS AND NO MORE. The arm below is the whole schema at this
    // level, and everything that is not one of the two is still refused BY
    // NAME, so a retired table and a plural typo both say what they are.
    for (key, value) in document {
        match key.as_str() {
            "recap" => config.recap = parse_recap(value)?,
            "plugins" => {
                let toml::Value::Table(plugins) = value else {
                    return Err(ConfigError::Invalid("`plugins` is not a table".to_string()));
                };

                for (name, entry) in plugins {
                    let toml::Value::Table(mut settings) = entry else {
                        return Err(ConfigError::Invalid(format!(
                            "plugin `{name}` is not a table"
                        )));
                    };
                    // `enabled` is removed rather than read, so the flag
                    // reaches this layer and everything left over reaches the
                    // plugin untouched.
                    let enabled = match settings.remove("enabled") {
                        None => false,
                        Some(toml::Value::Boolean(flag)) => flag,
                        Some(_) => {
                            return Err(ConfigError::Invalid(format!(
                                "plugin `{name}` has a non-boolean `enabled`"
                            )));
                        }
                    };
                    config
                        .plugins
                        .insert(name, PluginEntry { enabled, settings });
                }
            }
            _ => {
                return Err(ConfigError::Invalid(format!(
                    "unknown top-level key `{key}`"
                )));
            }
        }
    }
    Ok(config)
}

/// `[recap]`'s switches, each starting at its default and moved only by a key
/// that states it.
fn parse_recap(value: toml::Value) -> Result<Recap, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`recap` is not a table".to_string()));
    };
    let mut recap = Recap::default();
    for (key, setting) in table {
        // THE COUNT IS ITS OWN ARM, ahead of the switches, because it is the
        // one key here that is not a boolean. A negative or fractional value is
        // refused BY NAME rather than clamped: the operator asked for a
        // threshold, and a silently corrected one is a threshold they believe
        // they set.
        if key == "min_events" {
            let Some(count) = setting
                .as_integer()
                .and_then(|count| usize::try_from(count).ok())
            else {
                return Err(ConfigError::Invalid(format!(
                    "`recap` key `min_events` has type `{}`, not a count",
                    setting.type_str()
                )));
            };
            // AND ZERO IS REFUSED BY NAME TOO, for the same reason a negative
            // is. `counted.len() >= 0` is always true, so a zero threshold
            // recaps EVERY event, including one over an empty window, which is
            // the state `recap::NOTHING_HAPPENED` says the event path never
            // posts. An operator calibrating the knob downward gets a card and
            // a Discord recap on every event, each saying nothing was
            // recorded. One is the floor: it means "any activity at all".
            if count == 0 {
                return Err(ConfigError::Invalid(
                    "`recap` key `min_events` is 0, which is not a threshold; 1 is the floor"
                        .to_string(),
                ));
            }
            recap.min_events = count;
            continue;
        }
        let switch = match key.as_str() {
            "replay_card" => &mut recap.replay_card,
            "digest" => &mut recap.digest,
            "digest_as_thread" => &mut recap.digest_as_thread,
            _ => {
                return Err(ConfigError::Invalid(format!("unknown `recap` key `{key}`")));
            }
        };
        match setting {
            toml::Value::Boolean(flag) => *switch = flag,
            other => {
                return Err(ConfigError::Invalid(format!(
                    "`recap` key `{key}` has type `{}`, not boolean",
                    other.type_str()
                )));
            }
        }
    }
    Ok(recap)
}

/// The IO edge: read the file at `path` and hand its text to the parser.
pub fn load_config(path: &Path) -> Result<LoadOutcome, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_config(&text).map(LoadOutcome::Loaded),
        // A dangling symlink also reads NotFound, and chezmoi deploys configs
        // as symlinks: the entry is PRESENT with a wrong target, so only an
        // absent entry is Missing and the broken link is an error.
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Ok(LoadOutcome::Missing)
        }
        Err(error) => Err(ConfigError::Unreadable(format!(
            "{}: {error}",
            path.display()
        ))),
    }
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
        let err = parse_config("[plugins.moshi]\ntoken = \"SUPERSECRET\" trailing\n").unwrap_err();
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

    // --- the recap's switches -----------------------------------------------

    #[test]
    fn a_recap_table_is_read_rather_than_refused_and_each_switch_stands_alone() {
        // ONE KEY STATED, THE OTHER TWO UNTOUCHED. The three deliveries are
        // independent, so an operator who silenced the recap must not find
        // they also silenced the catch-up card, or the other way round.
        let config = parse_config("[recap]\ndigest = false\n").unwrap();
        assert!(!config.recap.digest, "the stated switch was read");
        assert!(config.recap.replay_card, "the card kept its default");
        assert!(config.recap.digest_as_thread, "the thread kept its default");
    }

    #[test]
    fn a_config_with_no_recap_table_leaves_every_switch_on() {
        // ABSENT IS ALL ON, which is what makes the table optional: a machine
        // that never writes one behaves exactly as it did before the table
        // existed. The direction is STATED rather than derived, because a
        // derived default is all-off, and that would silently take the
        // catch-up card away from every machine whose config predates this.
        let config = parse_config("[plugins.hue]\nenabled = true\n").unwrap();
        assert!(config.recap.replay_card, "the catch-up card");
        assert!(config.recap.digest, "the recap");
        assert!(config.recap.digest_as_thread, "the recap's own thread");
    }

    #[test]
    fn a_misspelled_recap_key_is_refused_by_name_rather_than_left_at_its_default() {
        // UNKNOWN KEYS REFUSE HERE, unlike a plugin's free-form settings, and
        // the difference is who reads them: a plugin table is handed to a
        // plugin this layer cannot judge, while this table is read here and
        // nowhere else. An unjudged key is a typo that leaves the switch ON
        // while the operator believes they turned it off.
        let err = parse_config("[recap]\nreplaycard = false\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("replaycard"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_non_boolean_recap_switch_is_refused_naming_the_key() {
        // `digest = "yes"` read as a switch is the same defect one level down
        // from a non-boolean `enabled`: the operator asked for something, did
        // not get it, and was told nothing.
        let err = parse_config("[recap]\ndigest = \"yes\"\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("digest"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_top_level_key_that_merely_looks_like_recap_is_still_refused_by_name() {
        // GUARD. Admitting `[recap]` admits ONE more key and nothing else:
        // the plural typo, newly plausible now that the singular parses, has
        // to name itself rather than sit there as a table nothing reads. The
        // retired `[home]` table's test guards the same arm from the other
        // side, and both must stay green as the arm grows.
        let err = parse_config("[recaps]\ndigest = false\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("recaps"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_non_table_recap_value_is_refused_naming_the_key() {
        // `recap = 5` is `plugins = 5` one table over: a value at the key the
        // switches hang off must refuse, never fall through to the all-on
        // default and leave the operator believing their file was read.
        //
        // THE ARM IS NAMED, not just the key. "unknown top-level key `recap`"
        // is what comes back when the admitting arm is gone entirely, and it
        // carries the word `recap` too: an assertion that asked only for the
        // name would pass for the refusal that says the table is not a
        // setting at all, which is a different fault with a different fix.
        let err = parse_config("recap = 5\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("recap"),
                    "the offender is named: {message}"
                );
                assert!(
                    message.contains("is not a table"),
                    "and it is the non-table arm rather than the unknown-key one: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn the_recaps_volume_threshold_is_a_count_the_operator_can_state() {
        // THE CALIBRATION KNOB. The locked threshold carries a tilde and has no
        // live measurement behind it, so it ships as a key defaulted to the
        // operator's own guess and the recap's header prints the real count
        // every time. One week of real recaps settles it without a rebuild.
        let config = parse_config("[recap]\nmin_events = 3\n").unwrap();
        assert_eq!(config.recap.min_events, 3, "the stated count was read");
        assert!(config.recap.digest, "and the switches kept their defaults");
        assert_eq!(
            parse_config("[plugins.hue]\nenabled = true\n")
                .unwrap()
                .recap
                .min_events,
            8,
            "an absent key is the operator's stated eight"
        );
    }

    #[test]
    fn a_volume_threshold_of_zero_is_refused_by_name_rather_than_read_as_every_event() {
        // ZERO IS NOT A THRESHOLD. `counted.len() >= 0` is always true, so it
        // recaps every single event, including one over an EMPTY window, which
        // is the one state the recap body says the event path never posts. An
        // operator calibrating the knob downward would get a card and a Discord
        // recap on every event, each saying nothing was recorded.
        let err = parse_config("[recap]\nmin_events = 0\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(message.contains("min_events"), "{message}");
                assert!(
                    message.contains('1'),
                    "the refusal names the floor rather than only the offence: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        assert_eq!(
            parse_config("[recap]\nmin_events = 1\n")
                .unwrap()
                .recap
                .min_events,
            1,
            "and one is accepted: it means any activity at all"
        );
    }

    #[test]
    fn a_volume_threshold_that_is_not_a_count_is_refused_naming_the_key() {
        // A STRING, A FRACTION AND A NEGATIVE are each a threshold the operator
        // asked for and would not get, and each has to say so rather than leave
        // the count silently at its default.
        for stated in ["\"eight\"", "8.5", "-1", "true"] {
            let err = parse_config(&format!("[recap]\nmin_events = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => assert!(
                    message.contains("min_events"),
                    "the offender is named for {stated}: {message}"
                ),
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
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
    fn a_dangling_config_symlink_is_an_error_never_missing() {
        // chezmoi deploys configs as symlinks: a broken link is a PRESENT
        // entry whose target is wrong, and reading it as "unconfigured"
        // would silently disable everything. Only a truly absent entry is
        // Missing.
        let link = std::env::temp_dir().join(format!("pns-config-dangling-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("pns-absent-target", &link).unwrap();
        let outcome = load_config(&link);
        std::fs::remove_file(&link).ok();
        match outcome {
            Err(ConfigError::Unreadable(message)) => {
                assert!(!message.is_empty(), "the path and cause are named")
            }
            other => panic!("expected Unreadable, got {other:?}"),
        }
    }

    #[test]
    fn an_unreadable_path_is_an_error_never_a_silent_unconfigured() {
        // A directory at the config path is the deterministic unreadable
        // case: it exists, so reporting Missing here would make a broken
        // path read as "unconfigured" and silently disable everything.
        let outcome = load_config(std::env::temp_dir().as_path());
        match outcome {
            Err(ConfigError::Unreadable(message)) => {
                assert!(!message.is_empty(), "the path and cause are named")
            }
            other => panic!("expected Unreadable, got {other:?}"),
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
