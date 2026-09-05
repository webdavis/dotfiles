//! The config edge: `~/.config/uu/config.toml` decides what runs.
//!
//! THE FILE SELECTS; it never defines. A lane runs only when its
//! `[lanes.<name>]` block exists, records post only when `[records]` exists,
//! and alerts leave the machine only when `[alerts]` exists. With no file at
//! all a bare `uu run` runs nothing, logs what it found and exits clean,
//! which is what makes a fresh install harmless; `uu run <lane>` still asks
//! for that lane by name and is refused with exit 1.
//!
//! FOUR TABLES, and this file owns the top level plus the two leaf blocks
//! whose whole content is a setting each. `schema` states what every table
//! serves and what shape one value takes, `schedule` reads the day and time,
//! and `lanes` is the registry with a module per kind.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config; a MISSING file is its own outcome,
//! distinct from both error and emptiness; a `[records]` block with no signing
//! key is refused rather than left as a record path that can never land.

pub mod lanes;
pub mod schedule;
pub mod schema;

#[cfg(test)]
mod probes;
#[cfg(test)]
mod shipped_template;

use std::path::{Path, PathBuf};

use schema::{admits, non_empty, table_of};

pub use lanes::brew::{DEFAULT_BREW, DEFAULT_MAS, DEFAULT_TAILSCALED};
pub use lanes::herdr::DEFAULT_HERDR_BINARY;
pub use lanes::uv::DEFAULT_UV_BINARY;
pub use lanes::{
    BrewLane, CommandLane, HerdrLane, LANE_TYPES, Lane, LaneKind, Lanes, NpmLane, Plugin, UvLane,
};
pub use schedule::{Schedule, WEEKDAY_NAMES};
pub use schema::{TABLE_KEYS, TOP_LEVEL};

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
            "schedule" => config.schedule = schedule::parse_schedule(value)?,
            "records" => config.records = Some(parse_records(value)?),
            "alerts" => config.alerts = Some(parse_alerts(value)?),
            "lanes" => config.lanes = lanes::parse_lanes(value)?,
            _ => {
                return Err(ConfigError::Invalid(format!(
                    "unknown top-level key `{key}`; the file serves {}",
                    schema::keys_of(TOP_LEVEL).unwrap_or_default().join(", ")
                )));
            }
        }
    }
    Ok(config)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{parsed, refusal};

    #[test]
    fn an_empty_config_runs_nothing_and_posts_nothing() {
        let config = parsed("");
        assert_eq!(config.lanes, Lanes::default());
        assert_eq!(config.records, None);
        assert_eq!(config.alerts, None);
    }

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
    fn a_malformed_file_is_a_loud_error_and_never_an_empty_config() {
        let detail = refusal("[lanes\n");
        assert!(!detail.is_empty());
        assert!(matches!(
            parse_config("[lanes\n"),
            Err(ConfigError::Malformed(_))
        ));
    }

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
    fn an_alerts_block_finds_the_engine_on_path_when_it_names_no_binary() {
        assert_eq!(
            parsed("[alerts]\n").alerts,
            Some(Alerts {
                binary: DEFAULT_ALERT_BINARY.to_string()
            })
        );
    }

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

    #[test]
    fn the_config_lives_under_the_xdg_config_directory() {
        assert_eq!(
            config_path("/home/x"),
            std::path::PathBuf::from("/home/x/.config/uu/config.toml")
        );
    }
}
