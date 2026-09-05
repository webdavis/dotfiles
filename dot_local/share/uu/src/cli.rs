//! The three things uu does, one module each, and the two answers all three
//! need first.

pub mod doctor;
pub mod run;
pub mod schedule;

use std::path::Path;

use unattended_upgrades::config::{Config, ConfigError, LoadOutcome, load_config};

/// The config, or `None` for a machine that has not written one. A refusal is
/// printed here and returned as an exit code, because every mode answers it
/// the same way: loudly, and without guessing.
fn loaded(path: &Path) -> Result<Option<Config>, i32> {
    match load_config(path) {
        Ok(LoadOutcome::Loaded(config)) => Ok(Some(config)),
        Ok(LoadOutcome::Missing) => Ok(None),
        Err(error) => {
            let what = match error {
                ConfigError::Malformed(_) => "is not valid TOML",
                ConfigError::Invalid(_) => "is not a config uu can use",
                ConfigError::Unreadable(_) => "could not be read",
            };
            eprintln!("uu: {} {what}: {}", path.display(), error.detail());
            Err(1)
        }
    }
}

/// The one sentence every mode prints when the environment names no home.
fn no_home() -> i32 {
    eprintln!("uu: HOME is not set, so there is no config to read");
    1
}
