//! The config file, which is the only thing that tells morning where anything
//! lives.
//!
//! NOTHING HERE IS HARDCODED TO ONE MACHINE. morning is installed with
//! `cargo install`, so a ledger path, an apply log, a recap directory and the
//! two commands are all named by the operator. A source the config leaves out
//! is not an error: the brief prints the section and says the source is not
//! configured, which is how a fresh machine reads before anything is set up.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long any one source gets before it is abandoned.
const DEFAULT_TIMEOUT_SECONDS: f32 = 8.0;
/// How much of a recap the brief shows.
const DEFAULT_RECAP_LINES: usize = 20;
/// How many rows any other section shows before it starts counting.
const DEFAULT_ROWS_PER_SECTION: usize = 8;

/// Everything morning reads out of `~/.config/morning/config.toml`.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    timeout_seconds: Option<f32>,
    rows_per_section: Option<usize>,
    pub apply_log: Option<PathSource>,
    pub ledger: Option<LedgerSource>,
    pub recap: Option<RecapSource>,
    pub pull_requests: Option<CommandSource>,
    pub tasks: Option<CommandSource>,
}

/// A source that is one file.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PathSource {
    pub path: PathBuf,
}

/// The ledger, plus the phrases that classify its open items.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LedgerSource {
    pub path: PathBuf,
    #[serde(default = "apply_markers")]
    pub apply_markers: Vec<String>,
    #[serde(default = "operator_markers")]
    pub operator_markers: Vec<String>,
}

/// The directory overnight recaps are written to.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecapSource {
    pub directory: PathBuf,
    #[serde(default = "recap_lines")]
    pub lines: usize,
}

/// A source that is a command, spawned with its own credentials.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandSource {
    pub command: Vec<String>,
}

fn apply_markers() -> Vec<String> {
    vec!["chezmoi apply".to_string()]
}

fn operator_markers() -> Vec<String> {
    vec!["operator".to_string()]
}

fn recap_lines() -> usize {
    DEFAULT_RECAP_LINES
}

impl Config {
    /// Reads a config file. An absent file is a config with no source in it,
    /// so a machine that has never been set up still gets a page.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("{}: {error}", path.display())),
            Ok(text) => toml::from_str(&text).map_err(|error| {
                format!("{}: {}", path.display(), error.message().replace('\n', " "))
            }),
        }
    }

    /// How many rows a section prints before it counts the remainder.
    pub fn rows_per_section(&self) -> usize {
        self.rows_per_section.unwrap_or(DEFAULT_ROWS_PER_SECTION)
    }

    /// The per-source deadline.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs_f32(self.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS))
    }
}

impl PathSource {
    /// The path, with a leading `~` resolved against `home`. `None` when the
    /// key was left empty, which the shipped config does for the sources that
    /// have no sensible default.
    pub fn resolve(&self, home: &Path) -> Option<PathBuf> {
        expand(&self.path, home)
    }
}

impl LedgerSource {
    pub fn resolve(&self, home: &Path) -> Option<PathBuf> {
        expand(&self.path, home)
    }
}

impl RecapSource {
    pub fn resolve(&self, home: &Path) -> Option<PathBuf> {
        expand(&self.directory, home)
    }
}

impl CommandSource {
    /// The command, or `None` when the list is empty.
    pub fn resolve(&self) -> Option<&[String]> {
        match self.command.is_empty() {
            true => None,
            false => Some(&self.command),
        }
    }
}

fn expand(path: &Path, home: &Path) -> Option<PathBuf> {
    let text = path.to_str()?;
    match text {
        "" => None,
        "~" => Some(home.to_path_buf()),
        _ => match text.strip_prefix("~/") {
            Some(rest) => Some(home.join(rest)),
            None => Some(PathBuf::from(text)),
        },
    }
}

#[cfg(test)]
mod tests;
