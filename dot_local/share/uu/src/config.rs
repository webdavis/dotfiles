//! The config edge: `~/.config/uu/config.toml` decides what runs.
//!
//! THE FILE SELECTS; it never defines. A lane runs only when its
//! `[lanes.<name>]` block exists, records post only when `[records]` exists,
//! and alerts leave the machine only when `[alerts]` exists. With no file at
//! all uu runs nothing, logs what it found and exits clean, which is what
//! makes a fresh install harmless.
//!
//! EVERY UNKNOWN KEY IS REFUSED BY NAME, and so is every lane whose `type`
//! this build does not serve, or that states none on a name that is not itself
//! a type. The failure that buys is the one a silent pass-through cannot
//! report: a lane spelled `[lanes.hedr]` is a week that quietly updates
//! nothing while the operator reads a config that looks right.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config; a MISSING file is its own outcome,
//! distinct from both error and emptiness; a `[records]` block with no signing
//! key is refused rather than left as a record path that can never land.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The lane TYPES this build knows how to run: the roster of BUILT-IN
/// adapters, never the roster of names an operator may declare. A lane's NAME
/// is the operator's own choice (a producer-API lane names itself whatever it
/// likes); this is the roster its `type` is judged against, so the refusal
/// that names an unknown type and the listing of what this build serves both
/// read the one table.
pub const LANE_TYPES: &[&str] = &["command", "herdr"];

/// Where the config lives for a given home directory. Pure, so the path rule
/// is testable without an environment.
pub fn config_path(home: &str) -> PathBuf {
    Path::new(home).join(".config/uu/config.toml")
}

/// The whole parsed file.
#[derive(Debug, Default, PartialEq)]
pub struct Config {
    pub schedule: Schedule,
    /// `[records]`, or None when the block is absent, which is records off.
    pub records: Option<Records>,
    /// `[alerts]`, or None when the block is absent, which is alerts off.
    pub alerts: Option<Alerts>,
    pub lanes: Lanes,
}

/// When `uu schedule render` says the job should run.
///
/// TWO TRUTHS, and this is the standalone one. A machine whose scheduler is
/// managed elsewhere (this repo tracks a launchd plist of its own) takes its
/// timing from there, and this block feeds only the rendered plist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// launchd's own `Weekday` numbering: Sunday is 0.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            weekday: DEFAULT_WEEKDAY,
            hour: DEFAULT_HOUR,
            minute: DEFAULT_MINUTE,
        }
    }
}

/// Sunday noon, the shipped schedule.
const DEFAULT_WEEKDAY: u8 = 0;
const DEFAULT_HOUR: u8 = 12;
const DEFAULT_MINUTE: u8 = 0;

/// The day names a config writes, in launchd's own numbering.
pub const WEEKDAY_NAMES: [(&str, u8); 7] = [
    ("sunday", 0),
    ("monday", 1),
    ("tuesday", 2),
    ("wednesday", 3),
    ("thursday", 4),
    ("friday", 5),
    ("saturday", 6),
];

/// `[records]`: where the weekly what-happened entry is posted, and the key it
/// is signed with.
///
/// THE KEY IS REQUIRED once the block exists. A records block that cannot sign
/// is a record path that can never land, and the whole point of the record is
/// that its absence means something.
#[derive(Debug, Clone, PartialEq)]
pub struct Records {
    pub url: String,
    pub key: String,
}

/// The gateway route the record goes to when no key states one.
pub const DEFAULT_RECORD_URL: &str = "http://127.0.0.1:8644/webhooks/unattended-upgrades";

/// `[alerts]`: the pns engine a failed lane is reported through.
#[derive(Debug, Clone, PartialEq)]
pub struct Alerts {
    pub binary: String,
}

/// The engine name when no key states a path: found on PATH, like every other
/// command uu runs.
pub const DEFAULT_ALERT_BINARY: &str = "pns";

/// The lane REGISTRY: every declared `[lanes.<name>]` block, keyed by the name
/// the operator chose. A `BTreeMap` orders lanes by NAME regardless of the
/// file's own order (`toml::Table` is itself a `BTreeMap`, and neither this
/// crate nor pns enables the `toml`/`indexmap` `preserve_order` feature, so a
/// run's sequence never depends on where a block happens to sit in the file).
pub type Lanes = BTreeMap<String, LaneKind>;

/// One lane's ADAPTER, selected by its `type` key (the house rule: `type`
/// selects backends everywhere). The kind carries the lane's own parsed
/// settings; behavior lives in `lanes::run_lane`'s one dispatch match, never
/// here.
///
/// `Command` is the PRODUCER API: a generic adapter that runs any executable
/// the block names, under the locked contract (a JSON run event on the
/// child's stdin, the exit code as the verdict). Its stdin is PRE-FILLED
/// before the child spawns, rather than written to it after, because uu
/// resets SIGPIPE to SIG_DFL at start-up (main.rs), and a write to a child's
/// stdin after spawn can kill uu with status 141 if that child exits without
/// reading it.
#[derive(Debug, Clone, PartialEq)]
pub enum LaneKind {
    Command(CommandLane),
    Herdr(HerdrLane),
}

impl LaneKind {
    /// The `type` value this variant was parsed from.
    pub fn type_name(&self) -> &'static str {
        match self {
            LaneKind::Command(_) => "command",
            LaneKind::Herdr(_) => "herdr",
        }
    }
}

/// `[lanes.<name>]` with `type = "command"`: `run[0]` is the program,
/// `run[1..]` its arguments. The lane's NAME is the operator's own choice;
/// nothing here constrains it, which is the whole point of the producer API.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandLane {
    pub run: Vec<String>,
}

/// `[lanes.herdr]`: the herdr binary to drive, and the plugin roster to
/// refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct HerdrLane {
    pub binary: String,
    pub plugins: Vec<Plugin>,
}

/// The herdr command when no key states one.
pub const DEFAULT_HERDR_BINARY: &str = "herdr";

/// One GitHub-sourced herdr plugin: the installed id, and the source to
/// reinstall it from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub repo: String,
}

/// Why a config could not be used. Every variant carries the offender by name,
/// because "config invalid" without a noun is a hunt.
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
    /// What went wrong, already sanitized for printing.
    pub fn detail(&self) -> &str {
        match self {
            ConfigError::Malformed(detail)
            | ConfigError::Invalid(detail)
            | ConfigError::Unreadable(detail) => detail,
        }
    }
}

/// What loading found at the path. `Missing` is deliberately not an error: an
/// unconfigured machine is a state to report, not a fault to diagnose.
#[derive(Debug, PartialEq)]
pub enum LoadOutcome {
    Missing,
    Loaded(Config),
}

/// EVERY KEY EVERY TABLE SERVES, table by table: the one statement of this
/// schema's vocabulary, and the source of both the refusal that names a
/// mistyped key and the list of what to write instead.
pub const TABLE_KEYS: &[(&str, &[&str])] = &[
    (TOP_LEVEL, &["alerts", "lanes", "records", "schedule"]),
    ("schedule", &["day", "time"]),
    ("records", &["key", "url"]),
    ("alerts", &["binary"]),
    ("lanes.command", &["run", "type"]),
    ("lanes.herdr", &["binary", "plugins", "type"]),
];

/// The roster row for the file's own top level. THE EMPTY NAME, because that
/// level has no heading an operator writes.
pub const TOP_LEVEL: &str = "";

/// What one table serves, or `None` for a table this schema has no vocabulary
/// for.
fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

/// Whether a table admits a key, refusing it BY NAME and with the whole
/// vocabulary spelled out when it does not. `table` is what the refusal NAMES
/// as the offender; `vocabulary` is which `TABLE_KEYS` row judges it. The two
/// differ only for a lane block, where an operator-chosen name (`lanes.mine`)
/// is judged against its TYPE's row (`lanes.herdr`); every other caller passes
/// the same string twice.
///
/// THIS IS THE ONE GATE, and every table's match then has a fallthrough that
/// does nothing. The alternative shape, a gate here AND a refusal in each
/// fallthrough, spells the same rule twice: removing either leaves the suite
/// green, so neither is pinned and the roster stops being load-bearing. The
/// drift this arrangement could still admit, a key declared in the roster with
/// no arm to read it, is what `every_key_the_roster_declares_is_actually_read`
/// walks.
fn admits(table: &str, vocabulary: &str, key: &str) -> Result<(), ConfigError> {
    match keys_of(vocabulary) {
        Some(serves) if !serves.contains(&key) => Err(unknown_key(table, vocabulary, key)),
        _ => Ok(()),
    }
}

/// The refusal itself, naming the offending table, the key, and the
/// vocabulary that answers for it. THE LISTING IS THE POINT: a refusal that
/// only says a key is unknown leaves an operator guessing at the spelling,
/// and guessing is what produced it. A lane's vocabulary is phrased around
/// its TYPE ("a `herdr` lane serves ...") because the table named in the
/// refusal may be a name the operator chose, not the type that governs it.
fn unknown_key(table: &str, vocabulary: &str, key: &str) -> ConfigError {
    let keys = keys_of(vocabulary).unwrap_or_default().join(", ");
    let whose = match vocabulary.strip_prefix("lanes.") {
        Some(kind) => format!("a `{kind}` lane"),
        None => "the table".to_string(),
    };
    ConfigError::Invalid(format!(
        "unknown `{table}` key `{key}`; {whose} serves {keys}"
    ))
}

/// Read the file at `path`. A file that is not there is `Missing`, not an
/// error; every other failure is named.
///
/// A DANGLING SYMLINK IS NOT AN ABSENT FILE, though the kernel reports both as
/// NotFound. chezmoi deploys configs as symlinks, so a broken link is a
/// CONFIGURED machine whose file stopped resolving, and reading that as an
/// unconfigured one turns every lane off without a word. The link itself is
/// what decides, exactly as the pns loader beside this one does it.
pub fn load_config(path: &Path) -> Result<LoadOutcome, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_config(&text).map(LoadOutcome::Loaded),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Ok(LoadOutcome::Missing)
        }
        Err(error) => Err(ConfigError::Unreadable(error.to_string())),
    }
}

/// The pure half: text in, config or a named refusal out.
pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    // The parser's Display echoes the offending source line and this file
    // carries the signing key, so the refusal is rebuilt from the cause and
    // the location alone.
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
    for (key, value) in document {
        match key.as_str() {
            "schedule" => config.schedule = parse_schedule(value)?,
            "records" => config.records = Some(parse_records(value)?),
            "alerts" => config.alerts = Some(parse_alerts(value)?),
            "lanes" => config.lanes = parse_lanes(value)?,
            _ => {
                return Err(ConfigError::Invalid(format!(
                    "unknown top-level key `{key}`; the file serves {}",
                    keys_of(TOP_LEVEL).unwrap_or_default().join(", ")
                )));
            }
        }
    }
    Ok(config)
}

fn parse_schedule(value: toml::Value) -> Result<Schedule, ConfigError> {
    let table = table_of("schedule", value)?;
    let mut schedule = Schedule::default();
    for (key, setting) in table {
        admits("schedule", "schedule", &key)?;
        match key.as_str() {
            "day" => schedule.weekday = weekday(&non_empty("schedule", &key, &setting)?)?,
            "time" => {
                let (hour, minute) = time_of_day(&non_empty("schedule", &key, &setting)?)?;
                schedule.hour = hour;
                schedule.minute = minute;
            }
            // `admits` above is the ONE gate; nothing reaches here.
            _ => {}
        }
    }
    Ok(schedule)
}

/// `day`, refused BY NAME outside the seven, because a day nothing matches is
/// a schedule the operator wrote and no plist can carry.
fn weekday(name: &str) -> Result<u8, ConfigError> {
    WEEKDAY_NAMES
        .iter()
        .find(|(spelling, _)| *spelling == name)
        .map(|(_, number)| *number)
        .ok_or_else(|| {
            let known: Vec<&str> = WEEKDAY_NAMES.iter().map(|(word, _)| *word).collect();
            ConfigError::Invalid(format!(
                "`schedule` key `day` is `{name}`, which is no day; it serves {}",
                known.join(", ")
            ))
        })
}

/// `time`, as `HH:MM` on a 24-hour clock.
///
/// THE SHAPE IS JUDGED RATHER THAN COERCED. `12` and `12:00:00` and `24:00`
/// each parse as something under a lenient reading, and each would render a
/// plist that runs at an hour nobody asked for.
fn time_of_day(stated: &str) -> Result<(u8, u8), ConfigError> {
    let refusal = || {
        ConfigError::Invalid(format!(
            "`schedule` key `time` is `{stated}`, which is not a 24-hour `HH:MM` time"
        ))
    };
    let (hour, minute) = stated.split_once(':').ok_or_else(refusal)?;
    if hour.len() != 2 || minute.len() != 2 {
        return Err(refusal());
    }
    // DIGITS ONLY. `u8::parse` on its own admits a leading `+`, so `+1:23`
    // would coerce to 01:23, an hour nobody wrote.
    if !hour
        .bytes()
        .chain(minute.bytes())
        .all(|byte| byte.is_ascii_digit())
    {
        return Err(refusal());
    }
    let hour: u8 = hour.parse().map_err(|_| refusal())?;
    let minute: u8 = minute.parse().map_err(|_| refusal())?;
    if hour > 23 || minute > 59 {
        return Err(refusal());
    }
    Ok((hour, minute))
}

fn parse_records(value: toml::Value) -> Result<Records, ConfigError> {
    let table = table_of("records", value)?;
    let mut url = DEFAULT_RECORD_URL.to_string();
    let mut key = None;
    for (name, setting) in table {
        admits("records", "records", &name)?;
        match name.as_str() {
            "url" => url = non_empty("records", &name, &setting)?,
            "key" => key = Some(non_empty("records", &name, &setting)?),
            // `admits` above is the ONE gate; nothing reaches here.
            _ => {}
        }
    }
    // A RECORDS BLOCK THAT CANNOT SIGN IS REFUSED, not quietly demoted to
    // log-only. The record's absence is what the operator reads as a dead
    // machine, so a path that can never post has to say so at load.
    let key = key.ok_or_else(|| {
        ConfigError::Invalid(
            "`records` has no `key`, so nothing it posts could be signed; remove the table to \
             switch records off"
                .to_string(),
        )
    })?;
    Ok(Records { url, key })
}

fn parse_alerts(value: toml::Value) -> Result<Alerts, ConfigError> {
    let table = table_of("alerts", value)?;
    let mut binary = DEFAULT_ALERT_BINARY.to_string();
    for (name, setting) in table {
        admits("alerts", "alerts", &name)?;
        // One key, so an `if` rather than the match the other tables use;
        // `admits` above is still the one gate and nothing else can arrive.
        if name == "binary" {
            binary = non_empty("alerts", &name, &setting)?;
        }
    }
    Ok(Alerts { binary })
}

/// `[lanes]`, whose blocks are keyed by an operator-chosen NAME and dispatch
/// on the TYPE each one states.
///
/// A TYPE IS REQUIRED, EXPLICIT OR IMPLIED, and an unrecognized one is
/// refused rather than ignored, which is the one place this file departs
/// from pns's free-form plugin settings. A lane declared but never dispatched
/// is a subject that silently never updates, and nothing else on the machine
/// would ever say so.
fn parse_lanes(value: toml::Value) -> Result<Lanes, ConfigError> {
    let table = table_of("lanes", value)?;
    let mut lanes = Lanes::new();
    for (name, block) in table {
        let table_label = format!("lanes.{name}");
        let fields = table_of(&table_label, block)?;
        let kind = match lane_type(&name, &table_label, &fields)?.as_str() {
            "command" => LaneKind::Command(parse_command_lane(&table_label, fields)?),
            "herdr" => LaneKind::Herdr(parse_herdr_lane(&table_label, fields)?),
            // `lane_type` never returns anything outside `LANE_TYPES`.
            _ => unreachable!("lane_type only answers a member of LANE_TYPES"),
        };
        lanes.insert(name, kind);
    }
    Ok(lanes)
}

/// The lane's TYPE: read from its own `type` key, or IMPLIED when the block
/// says nothing and the NAME itself is a built-in type.
///
/// ONLY A BUILT-IN NAME GETS THIS DEFAULT, narrower on purpose than pns's
/// "NOTHING GUESSES A BACKEND": it is what keeps a config written before the
/// producer API (a bare `[lanes.herdr]`, no `type`) working unchanged. A name
/// that is not a built-in type and states no `type` names nothing to dispatch
/// on, so it is refused rather than guessed.
fn lane_type(name: &str, table_label: &str, table: &toml::Table) -> Result<String, ConfigError> {
    match table.get("type") {
        Some(value) => {
            let stated = non_empty(table_label, "type", value)?;
            if LANE_TYPES.contains(&stated.as_str()) {
                Ok(stated)
            } else {
                Err(ConfigError::Invalid(format!(
                    "lane `{name}` has type `{stated}`, which is no lane type; this build serves \
                     {}",
                    LANE_TYPES.join(", ")
                )))
            }
        }
        None if LANE_TYPES.contains(&name) => Ok(name.to_string()),
        None => Err(ConfigError::Invalid(format!(
            "lane `{name}` names no `type`; this build serves {}",
            LANE_TYPES.join(", ")
        ))),
    }
}

fn parse_herdr_lane(table_label: &str, table: toml::Table) -> Result<HerdrLane, ConfigError> {
    let mut lane = HerdrLane {
        binary: DEFAULT_HERDR_BINARY.to_string(),
        plugins: Vec::new(),
    };
    for (name, setting) in table {
        admits(table_label, "lanes.herdr", &name)?;
        match name.as_str() {
            "binary" => lane.binary = non_empty(table_label, &name, &setting)?,
            "plugins" => lane.plugins = parse_plugins(&setting)?,
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    Ok(lane)
}

/// `[lanes.<name>]` with `type = "command"`: `run` is required, everything
/// else `admits` above already refused.
///
/// `admits` RUNS FIRST, inside this same loop, before the after-loop check
/// below for a missing `run`. A block that names an unknown key AND no `run`
/// (`[lanes.mine]\ntype = "command"\nbogus = 1`) is refused for the key it
/// misspelled, not for the run it never got to declare: the operator fixes
/// one problem at a time, and "unknown key" is the more specific diagnosis.
fn parse_command_lane(table_label: &str, table: toml::Table) -> Result<CommandLane, ConfigError> {
    let mut run = None;
    for (name, setting) in table {
        admits(table_label, "lanes.command", &name)?;
        match name.as_str() {
            "run" => run = Some(parse_run(table_label, &setting)?),
            // Read by `lane_type` before this block was dispatched; nothing
            // is left to do with it here.
            "type" => {}
            // `admits` above is the ONE gate; nothing else reaches here.
            _ => {}
        }
    }
    let run = run.ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{table_label}` has no `run`, so it names nothing to run"
        ))
    })?;
    Ok(CommandLane { run })
}

/// `run`: a non-empty list of non-blank strings. `run[0]` is the program that
/// gets executed and `run[1..]` its arguments, so a missing, wrongly-typed,
/// empty or blank entry each names nothing runnable and is refused by name.
fn parse_run(table_label: &str, setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let Some(entries) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `run` has type `{}`, not a list",
            setting.type_str()
        )));
    };
    if entries.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{table_label}` key `run` is empty, so it names nothing to run"
        )));
    }
    entries
        .iter()
        .map(|entry| match entry.as_str() {
            Some(word) if word.trim().is_empty() => Err(ConfigError::Invalid(format!(
                "`{table_label}` key `run` holds a blank entry, so it names nothing to run"
            ))),
            Some(word) => Ok(word.to_string()),
            None => Err(ConfigError::Invalid(format!(
                "`{table_label}` key `run` holds a `{}`, not a string",
                entry.type_str()
            ))),
        })
        .collect()
}

/// `plugins`, a list of `{ id, repo }` tables.
///
/// BOTH FIELDS ARE REQUIRED AND NEITHER MAY BE EMPTY. The refresh is an
/// uninstall by id followed by an install from the repo, so half an entry
/// uninstalls a plugin nothing can put back.
fn parse_plugins(setting: &toml::Value) -> Result<Vec<Plugin>, ConfigError> {
    let Some(entries) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`lanes.herdr` key `plugins` has type `{}`, not a list of plugins",
            setting.type_str()
        )));
    };
    entries
        .iter()
        .map(|entry| {
            let Some(fields) = entry.as_table() else {
                return Err(ConfigError::Invalid(format!(
                    "`lanes.herdr` key `plugins` holds a `{}`, not a plugin table",
                    entry.type_str()
                )));
            };
            for key in fields.keys() {
                if key != "id" && key != "repo" {
                    return Err(ConfigError::Invalid(format!(
                        "unknown `lanes.herdr` plugin key `{key}`; a plugin serves id, repo"
                    )));
                }
            }
            let field = |key: &str| {
                fields
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        ConfigError::Invalid(format!(
                            "`lanes.herdr` plugin entry has no usable `{key}` (missing, empty or \
                             only whitespace), so it names nothing to refresh"
                        ))
                    })
            };
            Ok(Plugin {
                id: field("id")?,
                repo: field("repo")?,
            })
        })
        .collect()
}

/// One table, refused BY NAME when the operator wrote something else there.
fn table_of(table: &str, value: toml::Value) -> Result<toml::Table, ConfigError> {
    match value {
        toml::Value::Table(entries) => Ok(entries),
        other => Err(ConfigError::Invalid(format!(
            "`{table}` has type `{}`, not a table",
            other.type_str()
        ))),
    }
}

/// One key that has to be a string with something in it. A value that is
/// empty, or nothing but whitespace, names nothing, so it is refused rather
/// than read as "use the default": the operator wrote a value, and silently
/// substituting another is a setting they believe they set. A blank one is the
/// worse of the two, because the file reads as though the setting was made.
fn non_empty(table: &str, key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    match setting.as_str() {
        Some(blank) if blank.trim().is_empty() => Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` is empty or only whitespace, so it names nothing"
        ))),
        Some(value) => Ok(value.to_string()),
        None => Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not a string",
            setting.type_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Config {
        parse_config(text).expect("this config is valid")
    }

    fn refusal(text: &str) -> String {
        match parse_config(text) {
            Err(error) => error.detail().to_string(),
            Ok(config) => panic!("this config should have been refused, got {config:?}"),
        }
    }

    // --- what an empty file means --------------------------------------------

    #[test]
    fn an_empty_config_runs_nothing_and_posts_nothing() {
        let config = parsed("");
        assert_eq!(config.lanes, Lanes::default());
        assert_eq!(config.records, None);
        assert_eq!(config.alerts, None);
    }

    #[test]
    fn the_shipped_schedule_is_sunday_at_noon() {
        assert_eq!(
            parsed("").schedule,
            Schedule {
                weekday: 0,
                hour: 12,
                minute: 0
            }
        );
    }

    // --- refusal by name ------------------------------------------------------

    #[test]
    fn an_unknown_top_level_key_is_refused_and_the_file_lists_what_it_serves() {
        let detail = refusal("[lane.herdr]\n");
        assert!(detail.contains("unknown top-level key `lane`"), "{detail}");
        assert!(
            detail.contains("alerts, lanes, records, schedule"),
            "{detail}"
        );
    }

    #[test]
    fn a_lane_named_after_no_built_in_type_and_naming_no_type_is_refused_by_name() {
        let detail = refusal("[lanes.hedr]\n");
        assert!(detail.contains("lane `hedr` names no `type`"), "{detail}");
        assert!(detail.contains("herdr"), "{detail}");
    }

    #[test]
    fn an_unknown_lane_type_is_refused_naming_it_and_the_known_types() {
        let detail = refusal("[lanes.mine]\ntype = \"hedr\"\n");
        assert!(detail.contains("lane `mine` has type `hedr`"), "{detail}");
        assert!(detail.contains("herdr"), "{detail}");
    }

    #[test]
    fn the_herdr_lane_still_parses_with_its_type_written_out() {
        assert_eq!(
            parsed("[lanes.herdr]\ntype = \"herdr\"\n")
                .lanes
                .get("herdr"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: DEFAULT_HERDR_BINARY.to_string(),
                plugins: Vec::new(),
            }))
        );
    }

    #[test]
    fn a_herdr_lane_may_carry_any_name_once_its_type_says_herdr() {
        assert_eq!(
            parsed("[lanes.mine]\ntype = \"herdr\"\n").lanes.get("mine"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: DEFAULT_HERDR_BINARY.to_string(),
                plugins: Vec::new(),
            }))
        );
    }

    #[test]
    fn every_lane_type_refuses_a_key_it_does_not_serve_and_so_does_a_user_named_lane() {
        // The built-in roster, judged by its own name...
        for kind in LANE_TYPES {
            let detail = refusal(&format!("[lanes.{kind}]\nbogus = 1\n"));
            assert!(
                detail.contains(&format!("unknown `lanes.{kind}` key `bogus`")),
                "{detail}"
            );
        }
        // ...and an operator-chosen name of that same type, judged the same
        // way: the table named in the refusal is the CHOSEN name, and the
        // vocabulary spelled out is still the TYPE's.
        let detail = refusal("[lanes.mine]\ntype = \"herdr\"\nbogus = 1\n");
        assert!(
            detail.contains("unknown `lanes.mine` key `bogus`"),
            "{detail}"
        );
        assert!(detail.contains("a `herdr` lane serves"), "{detail}");
    }

    #[test]
    fn the_templates_own_herdr_lane_shape_parses() {
        // dot_config/uu/private_config.toml.tmpl's [lanes.herdr] block,
        // verbatim, with its two templated values stood in for by a
        // placeholder: this cannot render headless (it reads the vault), so
        // this is what proves the SHAPE the template ships is one the parser
        // still accepts once `type` is added.
        let config = parsed(
            "[lanes.herdr]\n\
             type = \"herdr\"\n\
             binary = \"/home/example/.local/bin/herdr\"\n\
             plugins = [\n\
               { id = \"worktrunk\", repo = \"owner/herdr-worktrunk\" },\n\
             ]\n",
        );
        assert_eq!(
            config.lanes.get("herdr"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: "/home/example/.local/bin/herdr".to_string(),
                plugins: vec![Plugin {
                    id: "worktrunk".to_string(),
                    repo: "owner/herdr-worktrunk".to_string(),
                }],
            }))
        );
    }

    #[test]
    fn every_table_refuses_its_own_near_miss_and_lists_its_vocabulary() {
        for (text, table, key, serves) in [
            (
                "[schedule]\ndays = \"sunday\"\n",
                "schedule",
                "days",
                "day, time",
            ),
            (
                "[records]\nkey = \"k\"\nurls = \"u\"\n",
                "records",
                "urls",
                "key, url",
            ),
            ("[alerts]\nbin = \"pns\"\n", "alerts", "bin", "binary"),
            (
                "[lanes.herdr]\nplugin = []\n",
                "lanes.herdr",
                "plugin",
                "binary, plugins, type",
            ),
        ] {
            let detail = refusal(text);
            assert!(
                detail.contains(&format!("unknown `{table}` key `{key}`")),
                "{detail}"
            );
            assert!(detail.contains(serves), "{detail}");
        }
    }

    #[test]
    fn a_malformed_file_is_a_loud_error_and_never_an_empty_config() {
        let detail = refusal("[lanes\n");
        assert!(!detail.is_empty());
        assert!(matches!(
            parse_config("[lanes\n"),
            Err(ConfigError::Malformed(_))
        ));
    }

    #[test]
    fn a_table_written_as_a_scalar_is_refused_by_name() {
        for (text, table) in [
            ("schedule = 3\n", "schedule"),
            ("records = \"x\"\n", "records"),
            ("alerts = true\n", "alerts"),
            ("lanes = 1\n", "lanes"),
        ] {
            let detail = refusal(text);
            assert!(detail.contains(&format!("`{table}` has type")), "{detail}");
        }
    }

    // --- the schedule ---------------------------------------------------------

    #[test]
    fn each_day_name_maps_to_launchds_own_numbering_with_sunday_at_zero() {
        // THE PAIRS ARE WRITTEN OUT rather than read back off `WEEKDAY_NAMES`.
        // Walking the table to check the table is a tautology: it stays green
        // while every day renders a plist that fires on the wrong one.
        for (name, number) in [
            ("sunday", 0),
            ("monday", 1),
            ("tuesday", 2),
            ("wednesday", 3),
            ("thursday", 4),
            ("friday", 5),
            ("saturday", 6),
        ] {
            let config = parsed(&format!("[schedule]\nday = \"{name}\"\n"));
            assert_eq!(config.schedule.weekday, number, "case: {name}");
        }
        assert_eq!(WEEKDAY_NAMES.len(), 7, "a week has seven days");
    }

    #[test]
    fn a_day_that_is_no_day_is_refused_and_the_seven_are_listed() {
        // The near-misses cover each lenient matching a rewrite could reach
        // for: a typo, a capitalized day, a prefix, and a trailing stutter.
        // Every one of them names a day the operator can see, so accepting any
        // of them silently is a schedule they never wrote.
        for stated in ["sundae", "Sunday", "sun", "sundayy"] {
            let detail = refusal(&format!("[schedule]\nday = \"{stated}\"\n"));
            assert!(detail.contains(&format!("`{stated}`")), "{detail}");
            assert!(detail.contains("sunday, monday"), "{detail}");
            assert!(detail.contains("saturday"), "{detail}");
        }
    }

    #[test]
    fn a_time_is_read_as_hours_and_minutes_on_a_twenty_four_hour_clock() {
        let config = parsed("[schedule]\ntime = \"23:45\"\n");
        assert_eq!(config.schedule.hour, 23);
        assert_eq!(config.schedule.minute, 45);
        let midnight = parsed("[schedule]\ntime = \"00:00\"\n");
        assert_eq!((midnight.schedule.hour, midnight.schedule.minute), (0, 0));
    }

    #[test]
    fn a_time_that_is_not_hh_colon_mm_is_refused_rather_than_coerced() {
        // Each of these parses as SOMETHING under a lenient reading, and each
        // would render a plist that fires at an hour nobody asked for.
        for stated in [
            "12", "12:00:00", "24:00", "12:60", "9:00", "noon", "-1:00", "+1:23", "12:+5",
        ] {
            let detail = refusal(&format!("[schedule]\ntime = \"{stated}\"\n"));
            assert!(
                detail.contains("not a 24-hour `HH:MM` time"),
                "{stated}: {detail}"
            );
        }
    }

    // --- records --------------------------------------------------------------

    #[test]
    fn a_records_block_posts_to_the_unattended_upgrades_route_when_it_names_no_url() {
        let config = parsed("[records]\nkey = \"secret\"\n");
        assert_eq!(
            config.records,
            Some(Records {
                url: DEFAULT_RECORD_URL.to_string(),
                key: "secret".to_string(),
            })
        );
    }

    #[test]
    fn a_records_block_without_a_key_is_refused_because_it_could_never_post() {
        let detail = refusal("[records]\nurl = \"http://example/x\"\n");
        assert!(detail.contains("`records` has no `key`"), "{detail}");
    }

    #[test]
    fn an_empty_string_setting_is_refused_rather_than_read_as_the_default() {
        for (text, table, key) in [
            ("[records]\nkey = \"\"\n", "records", "key"),
            ("[records]\nkey = \"k\"\nurl = \"\"\n", "records", "url"),
            ("[alerts]\nbinary = \"\"\n", "alerts", "binary"),
            ("[lanes.herdr]\nbinary = \"\"\n", "lanes.herdr", "binary"),
            ("[schedule]\nday = \"\"\n", "schedule", "day"),
            ("[schedule]\ntime = \"\"\n", "schedule", "time"),
        ] {
            let detail = refusal(text);
            assert!(
                detail.contains(&format!("`{table}` key `{key}` is empty")),
                "{detail}"
            );
        }
    }

    #[test]
    fn a_setting_holding_only_whitespace_is_refused_the_same_way_an_empty_one_is() {
        // A space is not a value. `binary = " "` is a command nothing can run
        // and `url = " "` is a route nothing can post to, and both of them
        // LOOK set in the file, which is worse than a key that is not there:
        // the operator reads a config that says the setting is made.
        for (text, table, key) in [
            ("[records]\nkey = \" \"\n", "records", "key"),
            ("[records]\nkey = \"k\"\nurl = \" \"\n", "records", "url"),
            ("[alerts]\nbinary = \"\t\"\n", "alerts", "binary"),
            ("[lanes.herdr]\nbinary = \"  \"\n", "lanes.herdr", "binary"),
            ("[schedule]\nday = \" \"\n", "schedule", "day"),
            ("[schedule]\ntime = \"\t \"\n", "schedule", "time"),
        ] {
            let detail = refusal(text);
            assert!(
                detail.contains(&format!("`{table}` key `{key}` is empty")),
                "{detail}"
            );
        }
    }

    // --- alerts ---------------------------------------------------------------

    #[test]
    fn an_alerts_block_finds_the_engine_on_path_when_it_names_no_binary() {
        assert_eq!(
            parsed("[alerts]\n").alerts,
            Some(Alerts {
                binary: DEFAULT_ALERT_BINARY.to_string()
            })
        );
    }

    // --- the herdr lane -------------------------------------------------------

    #[test]
    fn a_lane_block_with_nothing_in_it_is_the_lane_on_with_its_defaults() {
        assert_eq!(
            parsed("[lanes.herdr]\n").lanes.get("herdr"),
            Some(&LaneKind::Herdr(HerdrLane {
                binary: DEFAULT_HERDR_BINARY.to_string(),
                plugins: Vec::new(),
            }))
        );
    }

    #[test]
    fn the_plugin_roster_is_read_as_id_and_repo_pairs_in_the_order_written() {
        let config = parsed(
            "[lanes.herdr]\n\
             plugins = [\n\
               { id = \"worktrunk\", repo = \"owner/herdr-worktrunk\" },\n\
               { id = \"herdr-bar\", repo = \"other/herdr-bar\" },\n\
             ]\n",
        );
        let Some(LaneKind::Herdr(herdr)) = config.lanes.get("herdr") else {
            panic!("expected a herdr lane, got {:?}", config.lanes.get("herdr"));
        };
        assert_eq!(
            herdr.plugins,
            vec![
                Plugin {
                    id: "worktrunk".to_string(),
                    repo: "owner/herdr-worktrunk".to_string(),
                },
                Plugin {
                    id: "herdr-bar".to_string(),
                    repo: "other/herdr-bar".to_string(),
                },
            ]
        );
    }

    #[test]
    fn half_a_plugin_entry_is_refused_because_a_refresh_uninstalls_before_it_installs() {
        for text in [
            "[lanes.herdr]\nplugins = [{ id = \"a\" }]\n",
            "[lanes.herdr]\nplugins = [{ repo = \"o/r\" }]\n",
            "[lanes.herdr]\nplugins = [{ id = \"\", repo = \"o/r\" }]\n",
            "[lanes.herdr]\nplugins = [{ id = \"a\", repo = \"\" }]\n",
            // A blank field is the same uninstall with the same nothing to
            // reinstall from, and it reads as a filled-in entry.
            "[lanes.herdr]\nplugins = [{ id = \" \", repo = \"o/r\" }]\n",
            "[lanes.herdr]\nplugins = [{ id = \"a\", repo = \"\t\" }]\n",
        ] {
            let detail = refusal(text);
            assert!(detail.contains("nothing to refresh"), "{detail}");
        }
    }

    #[test]
    fn a_plugin_entry_refuses_a_key_it_does_not_serve() {
        let detail =
            refusal("[lanes.herdr]\nplugins = [{ id = \"a\", repo = \"o/r\", pin = \"v1\" }]\n");
        assert!(
            detail.contains("unknown `lanes.herdr` plugin key `pin`"),
            "{detail}"
        );
    }

    #[test]
    fn a_plugin_list_that_is_not_a_list_of_tables_is_refused_by_name() {
        assert!(
            refusal("[lanes.herdr]\nplugins = \"worktrunk\"\n").contains("not a list of plugins")
        );
        assert!(
            refusal("[lanes.herdr]\nplugins = [\"worktrunk\"]\n").contains("not a plugin table")
        );
    }

    // --- the command lane -------------------------------------------------------

    #[test]
    fn a_command_lane_without_run_is_refused_because_it_names_nothing_to_run() {
        let detail = refusal("[lanes.command]\n");
        assert!(detail.contains("has no `run`"), "{detail}");
        assert!(detail.contains("names nothing to run"), "{detail}");
    }

    #[test]
    fn a_run_that_is_empty_not_a_list_or_holds_a_blank_is_refused_by_name() {
        for (text, expect) in [
            ("[lanes.command]\nrun = []\n", "is empty"),
            ("[lanes.command]\nrun = \"x\"\n", "not a list"),
            ("[lanes.command]\nrun = [1]\n", "not a string"),
            ("[lanes.command]\nrun = [\"\"]\n", "holds a blank entry"),
        ] {
            let detail = refusal(text);
            assert!(detail.contains(expect), "case {text:?}: {detail}");
        }
    }

    #[test]
    fn a_command_lane_reads_run_as_the_program_and_its_arguments() {
        let config = parsed("[lanes.mine]\ntype = \"command\"\nrun = [\"/bin/x\", \"--yes\"]\n");
        assert_eq!(
            config.lanes.get("mine"),
            Some(&LaneKind::Command(CommandLane {
                run: vec!["/bin/x".to_string(), "--yes".to_string()],
            }))
        );
        // A second way `type` could be ignored: a herdr-only key on a command
        // block must still be refused.
        let detail = refusal("[lanes.mine]\ntype = \"command\"\nrun = [\"x\"]\nbinary = \"y\"\n");
        assert!(
            detail.contains("unknown `lanes.mine` key `binary`"),
            "{detail}"
        );
    }

    #[test]
    fn a_command_lane_with_a_bogus_key_is_refused_as_unknown_before_the_run_check() {
        let detail = refusal("[lanes.command]\nbogus = 1\n");
        assert!(
            detail.contains("unknown `lanes.command` key `bogus`"),
            "{detail}"
        );
        assert!(!detail.contains("names nothing to run"), "{detail}");
    }

    // --- the file -------------------------------------------------------------

    #[test]
    fn a_path_with_nothing_at_it_is_missing_rather_than_an_error() {
        assert_eq!(
            load_config(Path::new("/nonexistent/uu-config-test.toml")),
            Ok(LoadOutcome::Missing)
        );
    }

    #[test]
    fn a_dangling_config_symlink_is_unreadable_rather_than_missing() {
        // chezmoi deploys configs as symlinks, and a broken link reads
        // NotFound exactly like an absent path. The two are opposite states:
        // an absent config is an unconfigured machine, a broken link is a
        // CONFIGURED machine whose file stopped resolving, and reading it as
        // "unconfigured" turns every lane off without a word.
        let link = std::env::temp_dir().join(format!("uu-config-dangling-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("uu-absent-target", &link).expect("the link");
        let outcome = load_config(&link);
        std::fs::remove_file(&link).ok();
        assert!(
            matches!(outcome, Err(ConfigError::Unreadable(_))),
            "{outcome:?}"
        );
    }

    // --- the path -------------------------------------------------------------

    #[test]
    fn the_config_lives_under_the_xdg_config_directory() {
        assert_eq!(
            config_path("/home/x"),
            std::path::PathBuf::from("/home/x/.config/uu/config.toml")
        );
    }

    // --- the roster -----------------------------------------------------------

    #[test]
    fn every_key_the_roster_declares_is_actually_read() {
        // The one drift a single admission gate can still admit: a key added to
        // `TABLE_KEYS` with no arm to read it is accepted and then ignored, so
        // the operator writes a setting that does nothing and no refusal ever
        // mentions it. Every key here is handed a value of the wrong type; an
        // arm refuses it, and a key with no arm accepts anything.
        for (table, keys) in TABLE_KEYS {
            for key in *keys {
                let text = if *table == TOP_LEVEL {
                    format!("{key} = 1\n")
                } else {
                    format!("[{table}]\n{key} = 1\n")
                };
                let detail = match parse_config(&text) {
                    Err(error) => error.detail().to_string(),
                    Ok(_) => panic!("`{table}` declares `{key}` and nothing reads it"),
                };
                // AND THE REFUSAL MUST NOT BE "UNKNOWN". A key nothing reads is
                // refused too, by the arm that catches everything else, so the
                // reason it gives is the whole difference between a key that is
                // read and one the roster merely advertises.
                assert!(
                    !detail.contains("unknown"),
                    "`{table}` declares `{key}` and nothing reads it: {detail}"
                );
            }
        }
    }

    #[test]
    fn every_built_in_lane_type_has_a_minimal_block_the_parser_accepts() {
        // One minimal, valid block per BUILT-IN TYPE (the WEEKDAY_NAMES
        // pattern): a type in the roster that the parser refuses would
        // advertise a lane nobody can turn on.
        let fixtures: &[(&str, &str)] = &[
            ("command", "[lanes.command]\nrun = [\"x\"]\n"),
            ("herdr", "[lanes.herdr]\n"),
        ];
        assert_eq!(LANE_TYPES.len(), fixtures.len());
        for (kind, text) in fixtures {
            assert!(
                parse_config(text).is_ok(),
                "the roster names `{kind}` but the parser refuses its minimal block"
            );
        }
    }
}
