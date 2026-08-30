//! The config edge: `~/.config/pns/config.toml` decides which plugins run.
//!
//! The file SELECTS; it never defines. Every plugin is compiled in, disabled
//! until its table says `enabled = true`, so a machine runs exactly what its
//! config names and nothing else. The settings inside a plugin's table are
//! free-form here: this layer proves the shape, the registry interprets the
//! contents, and neither knows the other's plugin names.
//!
//! `[recap]` is the one top-level table that is not a plugin, and the second
//! key admitted here: three booleans, two counts and one argument list THIS
//! layer reads itself. Because it reads them, it can judge them, so an unknown
//! key inside it, a count that is not a threshold, and a summarizer that is not
//! a list of command words are refused rather than passed along the way a
//! plugin's settings are.
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

/// The recap's three delivery switches, its volume threshold, and the command
/// it hands the window to.
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
/// `min_events` IS A KEY RATHER THAN A CONSTANT because nobody can calibrate it
/// yet: the locked volume threshold carries a tilde, and the machine it was
/// written for has no history to measure. The recap prints the window's real
/// count in its own header every time, so one week of real recaps settles the
/// number without a rebuild.
///
/// `summarizer` IS ARGV AND NEVER A SHELL STRING, which is what makes it a
/// backend switch rather than a plugin: nothing is interpreted, so there is no
/// quoting rule and no injection surface, and a different backend is simply a
/// different array. UNSET IS A WORKING SETTING, and the common one: with no
/// summarizer the recap posts the plain mechanical lists.
///
/// `repos` AND `review_notes` ARE THE TWO SOURCES PNS CANNOT FIND ON ITS OWN,
/// which is why they are keys and why an absent one is the working setting.
/// The engine knows project NAMES off a working directory and nothing about
/// which repository they are, and the review notes are one operator's own
/// pipeline directory. UNSET MEANS THE SOURCE IS NEVER READ AT ALL: no `gh` is
/// spawned and no directory is opened, which is the fence that makes both
/// sections opt-in rather than merely empty.
///
/// ONE NAMED VALUE, never a row of loose booleans. Four of the eight fields are
/// bools or counts; spread through a call they would sit adjacent and a swap
/// would go unnoticed, and named fields cannot be transposed. It is CLONE
/// rather than Copy only because the argv is a `Vec`, and the composition root
/// clones it once off a borrowed config.
#[derive(Debug, Clone, PartialEq)]
pub struct Recap {
    pub replay_card: bool,
    pub digest: bool,
    pub digest_as_thread: bool,
    pub min_events: usize,
    pub summarizer: Option<Vec<String>>,
    pub summarizer_deadline_secs: u64,
    pub repos: Vec<String>,
    pub review_notes: Option<String>,
}

impl Default for Recap {
    fn default() -> Self {
        Recap {
            replay_card: true,
            digest: true,
            digest_as_thread: true,
            min_events: DEFAULT_MIN_EVENTS,
            summarizer: None,
            summarizer_deadline_secs: DEFAULT_SUMMARIZER_DEADLINE_SECS,
            repos: Vec::new(),
            review_notes: None,
        }
    }
}

/// How many events a window needs before a recap is worth the operator's
/// attention. The operator's own stated figure; see `Recap`.
const DEFAULT_MIN_EVENTS: usize = 8;

/// How long the summarizer may take before the recap gives up on it and posts
/// the plain lists.
///
/// FOUR MINUTES, and it is generous on purpose. MEASURED on this machine:
/// `ollama run qwen3.5:4b` over the same prompt took 3m20s on a cold model load
/// and 9.3s warm. Nobody is waiting on it, because the caller is the detached
/// process the event path never joined; a deadline under the cold case would
/// turn every first recap after a reboot into the fallback.
///
/// ZERO IS ACCEPTED AND IS NOT A TRAP, unlike `min_events`'s zero. A deadline
/// of nothing simply cannot be met, so the recap falls to the plain lists and
/// SAYS it did, which is the same outcome as any other summarizer that does not
/// answer. Nothing silently changes shape, so there is nothing to refuse.
const DEFAULT_SUMMARIZER_DEADLINE_SECS: u64 = 240;

/// The most any summarizer may be given. ONE HOUR, which is fifteen times the
/// cold load the default covers, so no honest backend on any machine meets it;
/// see `seconds` for the two failures that live past it.
const MAX_SUMMARIZER_DEADLINE_SECS: u64 = 3600;

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
    // ONE ARM PER KEY, and the three that are not booleans read through a
    // function of their own, so each refusal sits with the shape it judges.
    for (key, setting) in table {
        match key.as_str() {
            "min_events" => recap.min_events = threshold(&setting)?,
            "repos" => recap.repos = repositories(&setting)?,
            "review_notes" => recap.review_notes = Some(note_glob(&setting)?),
            "summarizer" => recap.summarizer = Some(argv(&setting)?),
            "summarizer_deadline_secs" => recap.summarizer_deadline_secs = seconds(&setting)?,
            "replay_card" => recap.replay_card = flag(&key, &setting)?,
            "digest" => recap.digest = flag(&key, &setting)?,
            "digest_as_thread" => recap.digest_as_thread = flag(&key, &setting)?,
            _ => {
                return Err(ConfigError::Invalid(format!("unknown `recap` key `{key}`")));
            }
        }
    }
    Ok(recap)
}

/// One `[recap]` switch. A value of any other type is refused BY NAME rather
/// than read as its own truthiness, which is what would leave a delivery on
/// while its config said otherwise.
fn flag(key: &str, setting: &toml::Value) -> Result<bool, ConfigError> {
    setting.as_bool().ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`recap` key `{key}` has type `{}`, not boolean",
            setting.type_str()
        ))
    })
}

/// `min_events`, the volume threshold. A negative or fractional value is
/// refused BY NAME rather than clamped: the operator asked for a threshold, and
/// a silently corrected one is a threshold they believe they set.
fn threshold(setting: &toml::Value) -> Result<usize, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `min_events` has type `{}`, not a count",
            setting.type_str()
        )));
    };
    // AND ZERO IS REFUSED BY NAME TOO, for the same reason a negative is.
    // `counted.len() >= 0` is always true, so a zero threshold recaps EVERY
    // event, including one over an empty window, which is the state
    // `recap::NOTHING_HAPPENED` says the event path never posts. An operator
    // calibrating the knob downward gets a card and a Discord recap on every
    // event, each saying nothing was recorded. One is the floor: it means "any
    // activity at all".
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`recap` key `min_events` is 0, which is not a threshold; 1 is the floor".to_string(),
        ));
    }
    Ok(count)
}

/// `summarizer_deadline_secs`, in whole seconds. See `min_events` for why a
/// value that is not a count is refused rather than corrected; zero is a
/// deadline nothing can meet, which is the fallback saying so, not a shape
/// this layer has to judge.
///
/// THE TOP END IS REFUSED BY NAME TOO, and unlike zero it really is a shape
/// this layer has to judge. Two things break past the ceiling and neither is
/// visible where it happens. NOTHING SUPERVISES THE DETACHED RECAP CHILD, which
/// `spawn_recap` states outright: at four minutes that is fine, and at a day it
/// is one child plus one wedged backend held for a day, with a second pair
/// arriving at the next return moment. AND `9223372036854775807` IS A PLAIN
/// TOML INTEGER: it parses, and `Instant::now() + Duration::from_secs` of it
/// PANICS (MEASURED: "overflow when adding duration to instant") inside a
/// process whose stderr is /dev/null and whose exit code nobody reads, so the
/// recap simply vanishes after the card has said it is coming. A refusal the
/// operator reads beats a silence they cannot.
fn seconds(setting: &toml::Value) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `summarizer_deadline_secs` has type `{}`, not a count of seconds",
            setting.type_str()
        )));
    };
    if count > MAX_SUMMARIZER_DEADLINE_SECS {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `summarizer_deadline_secs` is {count}, past the \
             {MAX_SUMMARIZER_DEADLINE_SECS}-second ceiling"
        )));
    }
    Ok(count)
}

/// `summarizer`, the command the window is handed to: a list of WORDS, passed
/// to the process directly and never through a shell.
///
/// THE SHELL STRING IS THE MISTAKE THIS REFUSES. `summarizer = "ollama run
/// qwen3.5:4b"` is what a hand writes first, and reading it as a one-word
/// command would name a binary nobody has, so it is refused by name rather than
/// left to fail once a night inside a detached process.
///
/// AN EMPTY LIST IS REFUSED TOO, because it names no command at all: taken as
/// written it would leave the summarizer configured and unrunnable, which reads
/// to the operator as a summarizer that is not answering rather than as a table
/// they have to fix.
///
/// AND SO IS AN EMPTY FIRST WORD, on that same reasoning rather than a new one.
/// `[""]` parses, `Command::new("")` fails to spawn, and the operator gets
/// precisely the outcome the paragraph above exists to prevent. Only the first
/// word is judged: an empty ARGUMENT is a real thing to pass a program, and
/// nothing about it stops the command running.
fn argv(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let words = strings("summarizer", "a list of command words", setting)?;
    if words.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `summarizer` is empty, so it names no command to run".to_string(),
        ));
    }
    if words[0].is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `summarizer` starts with an empty word, so it names no command to run"
                .to_string(),
        ));
    }
    Ok(words)
}

/// `repos`, the repositories the merged pull requests are read from: a list of
/// names in `gh`'s own `OWNER/REPO` spelling, passed to it as one argument
/// each.
///
/// EMPTINESS IS REFUSED AT BOTH LEVELS, for `summarizer`'s reason rather than a
/// new one. A key present with no name under it, or a name that is the empty
/// string, would leave the section reading "nothing merged in this window" over
/// a night that merged plenty, and the operator would be looking at their
/// repository rather than at their config.
///
/// THE NAME ITSELF IS NOT JUDGED BEYOND THAT, and deliberately. `gh` accepts
/// `OWNER/REPO`, `HOST/OWNER/REPO` and a full URL, it is the authority on which
/// of those exist, and a shape rule written here would refuse a spelling that
/// works. It is passed as ARGV, so nothing in it can be read as syntax by
/// anything; a name `gh` does not know costs the section one "unavailable"
/// line, which is the same rung a missing `gh` takes.
fn repositories(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let names = strings("repos", "a list of repository names", setting)?;
    if names.is_empty() || names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`recap` key `repos` names no repository to read".to_string(),
        ));
    }
    Ok(names)
}

/// One `[recap]` key holding a list of plain strings, with the key and what the
/// list is FOR named in every refusal. The emptiness rules belong to the
/// callers, because what an empty list MEANS is theirs.
fn strings(key: &str, noun: &str, setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let Some(values) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `{key}` has type `{}`, not {noun}",
            setting.type_str()
        )));
    };
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`recap` key `{key}` has a `{}` in it, not {noun}",
                    value.type_str()
                ))
            })
        })
        .collect()
}

/// `review_notes`, the one pattern deciding which files the recap may open.
///
/// THE GLOB IS THE WHOLE PERMISSION, which is why its shape is judged here
/// rather than resolved generously at the read. Two spellings are refused by
/// name and each would widen what pns opens beyond what the operator wrote:
///
/// A RELATIVE PATH resolves against the working directory, and the recap is
/// rendered by a process started from whatever directory the return event fired
/// in. The same key would then name a different set of files on every run, so
/// only an absolute path and a `~/` one are admitted.
///
/// AND A `*` IN A DIRECTORY makes the set of DIRECTORIES a search rather than a
/// statement: `~/.claude/*/checklist-*.md` asks pns to walk directories nobody
/// listed. Only the file name may hold one, which keeps the read to a single
/// directory the operator named in full.
fn note_glob(setting: &toml::Value) -> Result<String, ConfigError> {
    let Some(pattern) = setting.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` has type `{}`, not a path with a file name in it",
            setting.type_str()
        )));
    };
    let (directory, name) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if name.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `review_notes` names no file to read".to_string(),
        ));
    }
    if !pattern.starts_with('/') && !pattern.starts_with("~/") {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, which is not an absolute path or a `~/` one"
        )));
    }
    if directory.contains('*') {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, and only its file name may hold a `*`"
        )));
    }
    // AND EXACTLY ONE OF THEM, because that is all the matcher reads. A second
    // `*` is matched LITERALLY, so a pattern carrying one silently matches
    // nothing at all, which is the outcome every refusal in this file exists to
    // turn into a sentence the operator can act on.
    if name.matches('*').count() > 1 {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, and its file name may hold only one `*`"
        )));
    }
    Ok(pattern.to_string())
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

    #[test]
    fn the_summarizer_is_an_argument_list_the_operator_states_word_by_word() {
        // ARGV, NEVER A SHELL STRING. Nothing here is interpreted, so there is
        // no quoting rule to get wrong and no injection surface at all; a
        // different backend is a different array and a Linux machine writes its
        // own, which is the whole of the configurable-backend mandate.
        let config = parse_config(
            "[recap]\nsummarizer = [\"ollama\", \"run\", \"qwen3.5:4b\", \"--think=false\"]\n",
        )
        .unwrap();
        assert_eq!(
            config.recap.summarizer.as_deref(),
            Some(
                ["ollama", "run", "qwen3.5:4b", "--think=false"]
                    .map(String::from)
                    .as_slice()
            ),
            "the words the operator wrote, in order"
        );
        assert_eq!(
            parse_config("[recap]\ndigest = true\n")
                .unwrap()
                .recap
                .summarizer,
            None,
            "UNSET IS THE WORKING SETTING: no summarizer is the plain lists"
        );
    }

    #[test]
    fn a_summarizer_that_is_not_a_list_of_words_is_refused_naming_the_key() {
        // THE FOUR SHAPES A HAND WRITES BY MISTAKE: the shell string this key
        // deliberately is not, an array with something that is not a word in
        // it, an empty array that names no command at all, and an array whose
        // FIRST WORD is empty, which names no command either and used to parse.
        // `Command::new("")` fails to spawn, and the operator then reads a
        // summarizer that is not answering rather than the table they have to
        // fix, which is the exact outcome the empty-array refusal exists to
        // prevent. Each is refused by name rather than leaving the summarizer
        // silently unset, which is the difference between a recap that says it
        // fell back and one the operator believes is summarized.
        for (stated, expected) in [
            ("\"ollama run qwen3.5:4b\"", "not a list"),
            ("[\"ollama\", 3]", "not a list"),
            ("[]", "names no command"),
            ("[\"\"]", "names no command"),
            ("[\"\", \"run\"]", "names no command"),
        ] {
            let err = parse_config(&format!("[recap]\nsummarizer = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => {
                    assert!(
                        message.contains("summarizer"),
                        "the offender is named for {stated}: {message}"
                    );
                    assert!(
                        message.contains(expected),
                        "the refusal says what is wrong for {stated}: {message}"
                    );
                }
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_summarizers_deadline_is_a_count_of_seconds_defaulted_to_a_cold_model_load() {
        // FOUR MINUTES, because a cold `ollama` load MEASURED 3m20s on this
        // machine against 9.3s warm, and nobody is waiting: the caller is a
        // process the event path never joined.
        assert_eq!(
            parse_config("[recap]\ndigest = true\n")
                .unwrap()
                .recap
                .summarizer_deadline_secs,
            240,
            "the default covers a cold model load"
        );
        assert_eq!(
            parse_config("[recap]\nsummarizer_deadline_secs = 5\n")
                .unwrap()
                .recap
                .summarizer_deadline_secs,
            5
        );
        // AND IT HAS A TOP END, refused by name for `min_events`'s own reason.
        // An hour is already far past the cold load the default covers, and past
        // it the two failures are real: nothing supervises the detached recap
        // child, so a wedged backend holds one child and one backend process for
        // as long as the number says, and `9223372036854775807` is a plain TOML
        // integer that PANICS the child at `Instant::now() + deadline`
        // (MEASURED: "overflow when adding duration to instant"). That panic
        // lands in a process whose stderr is /dev/null and whose exit code
        // nobody reads, so the recap vanishes with no rung of the ladder taken,
        // after the card has already said it is coming.
        assert_eq!(
            parse_config("[recap]\nsummarizer_deadline_secs = 3600\n")
                .unwrap()
                .recap
                .summarizer_deadline_secs,
            3600,
            "an hour is inside the ceiling"
        );
        for stated in ["\"soon\"", "9.5", "-1", "3601", "9223372036854775807"] {
            let err = parse_config(&format!("[recap]\nsummarizer_deadline_secs = {stated}\n"))
                .unwrap_err();
            match err {
                ConfigError::Invalid(message) => assert!(
                    message.contains("summarizer_deadline_secs"),
                    "the offender is named for {stated}: {message}"
                ),
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_two_external_sources_are_named_by_the_operator_or_not_read_at_all() {
        // NEITHER SECTION HAS A SOURCE pns can find on its own: merged pull
        // requests live in a repository nothing here knows the name of, and the
        // review notes live wherever this operator's own pipeline puts them.
        // Both are therefore keys, and an absent key is the working setting.
        let config = parse_config(
            "[recap]\nrepos = [\"webdavis/dotfiles\"]\n\
             review_notes = \"~/.claude/pipeline/slices/checklist-*.md\"\n",
        )
        .unwrap();
        assert_eq!(config.recap.repos, ["webdavis/dotfiles".to_string()]);
        assert_eq!(
            config.recap.review_notes.as_deref(),
            Some("~/.claude/pipeline/slices/checklist-*.md")
        );
        let unconfigured = parse_config("[recap]\ndigest = true\n").unwrap().recap;
        assert!(
            unconfigured.repos.is_empty(),
            "UNSET IS THE WORKING SETTING: no repo is no `gh` at all"
        );
        assert_eq!(
            unconfigured.review_notes, None,
            "and no glob is no directory read at all"
        );
    }

    #[test]
    fn a_repos_value_that_is_not_repository_names_is_refused_naming_the_key() {
        // THE SAME FOUR SHAPES `summarizer` REFUSES, for the same reason: a
        // list this layer reads itself is a list it can judge, and a repo name
        // it silently dropped would read to the operator as a night with no
        // merges in it rather than as a table they have to fix.
        for (stated, expected) in [
            ("\"webdavis/dotfiles\"", "not a list"),
            ("[\"webdavis/dotfiles\", 3]", "not a list"),
            ("[]", "names no repository"),
            ("[\"\"]", "names no repository"),
        ] {
            let err = parse_config(&format!("[recap]\nrepos = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => {
                    assert!(
                        message.contains("repos"),
                        "the offender is named for {stated}: {message}"
                    );
                    assert!(
                        message.contains(expected),
                        "the refusal says what is wrong for {stated}: {message}"
                    );
                }
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_review_notes_glob_that_names_no_readable_file_is_refused_naming_the_key() {
        // THE GLOB IS THE WHOLE PERMISSION. It is the only thing that decides
        // which files pns opens, so a shape it cannot resolve exactly is
        // refused rather than resolved generously: a RELATIVE path would
        // resolve against whatever directory the return event happened to be
        // in, and a `*` in a DIRECTORY would make the set of directories pns
        // reads a search rather than a statement.
        for (stated, expected) in [
            ("3", "not a path"),
            ("\"\"", "names no file"),
            ("\"slices/checklist-*.md\"", "absolute"),
            ("\"~/.claude/*/checklist-*.md\"", "file name may hold a"),
            ("\"~/.claude/checklist-*-*.md\"", "only one"),
        ] {
            let err = parse_config(&format!("[recap]\nreview_notes = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => {
                    assert!(
                        message.contains("review_notes"),
                        "the offender is named for {stated}: {message}"
                    );
                    assert!(
                        message.contains(expected),
                        "the refusal says what is wrong for {stated}: {message}"
                    );
                }
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
