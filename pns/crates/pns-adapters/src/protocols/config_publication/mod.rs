use crate::now_secs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
mod backup_name;
pub use backup_name::backup_path;
mod check;
pub use check::check_config_path;
mod backup;
use backup::keep_aside;
mod restore;
use restore::restore_after_failure;

/// Publish the composed config, keeping the old one when replacing it.
///
/// CREATE-IF-ABSENT, NEVER A BLANKET RENAME, on both paths: a config that
/// appeared between the check in `setup_mode` and this moment is another
/// writer's, and this run has not read it. The link failing with
/// `AlreadyExists` IS that refusal. NOTHING ASKS WHETHER A CONFIG IS THERE
/// either, because the answer stops being true the instant it is given: what
/// `--force` moves aside is the file it found at the name, and what it
/// publishes into is a name it emptied itself.
///
/// THE OLD CONFIG IS MOVED ASIDE RATHER THAN COPIED ASIDE, so the backup holds
/// what was actually replaced rather than what stood there when a copy ran, and
/// the old config is at one of the two names at every instant.
///
/// THE PENDING FILE CARRIES THE MODE, because it is what gets published:
/// writing at the umask would publish a config whose plugin secrets any
/// process on the machine can read.
pub fn publish_config(path: &Path, composed: &str, force: bool) -> Result<Option<PathBuf>, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory to write in", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("{} could not be created: {error}", parent.display()))?;
    let pending = parent.join(pending_name());
    // CREATED OR NOT AT ALL, and never opened. A pending file is a second name
    // for the live config between the link that publishes it and the unlink
    // that removes it, so an abandoned run leaves one behind and process ids
    // are reused: an open that truncates would empty a config this run has not
    // read, and the backup taken next would hold the replacement.
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&pending)
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;
    let published = write_then_publish(path, &pending, file, composed, force);
    // WHICHEVER WAY IT WENT, and only ever the file the line above made: a
    // pending file left in the config directory would be read by nobody and
    // found by everybody, and removing one this run did not create is the
    // mirror of the write it refuses to do.
    let _ = std::fs::remove_file(&pending);
    published
}

/// The name the composed config is written under before it is published.
///
/// THE MOMENT AS WELL AS THE PROCESS, because the create above is exclusive: a
/// leftover from an abandoned run of the same id would otherwise refuse a
/// wizard nobody can unblock, and a name nothing else is holding is also a
/// name nothing else can be waiting at.
fn pending_name() -> String {
    format!(
        "config.toml.new.{}.{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.subsec_nanos())
    )
}

/// The publish itself, with `publish_config` owning the cleanup around it.
fn write_then_publish(
    path: &Path,
    pending: &Path,
    mut file: std::fs::File,
    composed: &str,
    force: bool,
) -> Result<Option<PathBuf>, String> {
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: the mode an
    // open asks for is masked by the umask, and a config published without the
    // operator's own bits is one they cannot read.
    file.set_permissions(std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
        .map_err(|error| format!("{} could not be secured: {error}", pending.display()))?;
    file.write_all(composed.as_bytes())
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;

    // THE FORCED PATH EMPTIES THE NAME FIRST, and what it moves out of the way
    // is the backup. Nothing here asks whether a config is there: the move
    // itself is the answer, and it is the same answer a moment later.
    let kept = if force { keep_aside(path)? } else { None };
    // AND BOTH PATHS PUBLISH THE SAME WAY. A link that refuses an occupied
    // name cannot write over a config this run never read: after the dangling
    // symlink pre-check in `setup_mode`, the only way a config can be
    // standing here is a genuine arrival while the questions were being
    // answered, so "appeared" below is exact rather than one of two guesses.
    match std::fs::hard_link(pending, path) {
        Ok(()) => Ok(kept),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
            "{} appeared while the questions were being answered; \
             nothing was written over it{}",
            path.display(),
            restore_after_failure(path, kept.as_deref())
        )),
        Err(error) => Err(format!(
            "{} could not be written: {error}{}",
            path.display(),
            restore_after_failure(path, kept.as_deref())
        )),
    }
}

/// The config carries every plugin's secret, so it is the operator's alone.
const CONFIG_FILE_MODE: u32 = 0o600;

#[cfg(test)]
mod tests;
