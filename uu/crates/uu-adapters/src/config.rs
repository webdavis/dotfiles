//! The config edge: `~/.config/uu/config.toml` decides what runs.
//!
//! THE FILE SELECTS; it never defines. A lane runs only when its
//! `[lanes.<name>]` block exists, records post only when `[records]` exists,
//! and alarms leave the machine only through configured destinations. With no file at
//! all a bare `uu run` runs nothing, logs what it found and exits clean,
//! which is what makes a fresh install harmless; `uu run <lane>` still asks
//! for that lane by name and is refused with exit 1.
//!
//! This file owns the top level and the alert-engine setting. `schema` states
//! the shared table vocabulary, `records` parses record and alarm destinations,
//! `schedule` reads the day and time, and `lanes` selects registered adapters.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config; a MISSING file is its own outcome,
//! distinct from both error and emptiness; a `[records]` block with no signing
//! key is refused rather than left as a record path that can never land.

mod lanes;
pub use lanes::{CargoLane, RotateLogsLane};

pub use lanes::SkillsConfig;
mod records;
pub use records::Records;
use records::parse_records;
mod escalation;
mod schedule;
mod schema;

use crate::LaneRegistration;
use std::path::{Path, PathBuf};

use schema::{admits, non_empty, table_of};

pub(crate) use lanes::NvimHost;
pub use lanes::{
    BrewLane, ClaudePluginsLane, CommandLane, HerdrLane, Lanes, NpmLane, NvimMasonLane,
    NvimParsersLane, NvimPluginsLane, NvimSmokeTestLane, UvLane,
};
pub(crate) use lanes::{
    parse_brew_lane, parse_cargo_lane, parse_command_lane, parse_herdr_lane, parse_npm_lane,
    parse_nvim_mason_lane, parse_nvim_parsers_lane, parse_nvim_plugins_lane, parse_uv_lane,
};
pub use schedule::Schedule;
use schema::TOP_LEVEL;

/// Where the config lives for a given home directory. Pure, so the path rule
/// is testable without an environment.
pub fn config_path(home: &str) -> PathBuf {
    Path::new(home).join(".config/uu/config.toml")
}

/// The whole parsed file.
#[derive(Debug, Default)]
pub struct Config {
    pub schedule: Schedule,
    /// `[records]`, or None when the block is absent, which is records off.
    pub records: Option<Records>,
    /// `[alerts]`, or None when the block is absent, which is alerts off.
    pub alerts: Option<Alerts>,
    pub lanes: Lanes,
}

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
#[derive(Debug)]
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
pub fn load_config(
    path: &Path,
    registrations: &[LaneRegistration],
) -> Result<LoadOutcome, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_config(&text, registrations).map(LoadOutcome::Loaded),
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
pub(crate) fn parse_config(
    text: &str,
    registrations: &[LaneRegistration],
) -> Result<Config, ConfigError> {
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
            "lanes" => config.lanes = lanes::parse_lanes(value, registrations)?,
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
use records::DEFAULT_RECORD_URL;

#[cfg(test)]
pub(crate) use lanes::{DEFAULT_BREW, DEFAULT_MAS, DEFAULT_TAILSCALED, Plugin};

#[cfg(test)]
mod probes;
#[cfg(test)]
pub(crate) use probes::parse_config as parse_test_config;
#[cfg(test)]
mod shipped_template;

#[cfg(test)]
mod tests;
