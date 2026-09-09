//! Which hosts file this run reconciles.
//!
//! THE SEAM EXISTS SO THE REAL FILE CAN BE LEFT ALONE while this is exercised,
//! and it has THREE states rather than two. An environment variable that is SET
//! BUT EMPTY is not the same as one that is unset: reading them the same way is
//! what aims a root rewrite at the real `/etc/hosts` when a caller meant to
//! point it somewhere else and produced an empty path.

use std::path::PathBuf;

/// The variable a caller sets to reconcile a file that is not the system's.
pub(crate) const VARIABLE: &str = "TAILNET_PIN_HOSTS_FILE";

/// The file this reconciles when nothing says otherwise.
pub(crate) const DEFAULT: &str = "/etc/hosts";

/// The path to reconcile, or the refusal for a variable set to nothing.
pub(crate) fn hosts_file_path() -> Result<PathBuf, String> {
    match std::env::var_os(VARIABLE) {
        None => Ok(PathBuf::from(DEFAULT)),
        Some(value) if value.is_empty() => Err(format!(
            "refusing to edit any hosts file: {VARIABLE} is set but EMPTY, which is not the same as unset; unset it to reconcile {DEFAULT}, or give it a path"
        )),
        Some(value) => Ok(PathBuf::from(value)),
    }
}
