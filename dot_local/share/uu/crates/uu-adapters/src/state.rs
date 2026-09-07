//! uu's own bookkeeping under `~/.local/state/uu`: the run lock, the
//! last-successful-run marker, and one non-success streak per lane.
//!
//! THREE MECHANISMS, three files, one directory. Nothing here decides
//! anything: the policy lives in the library (`record::marker`, `staleness`)
//! and this is where it is read from and written to disk.

pub mod lock;
pub mod marker;
pub mod streak;

use std::path::{Path, PathBuf};

/// Where every file below lives.
pub fn dir(home: &str) -> PathBuf {
    Path::new(home).join(".local/state/uu")
}

/// A path of a test's own under the temp directory, with nothing at it.
#[cfg(test)]
pub(crate) fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("uu-state-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}
