use super::limits::STATE_FILE_MODE;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Publish one line to a state file, atomically. The error is returned rather
/// than swallowed, so each caller states its own fail direction: a background
/// warning drops it, and a human waiting on a typed command hears about it.
///
/// PUBLISHED BY RENAME, the way the turn marker's claim is claimed further
/// down. A plain write truncates first, so a reader landing between the
/// truncate and the bytes sees an empty file, which every reader of these
/// files reads as no state at all. The pending path sits in the SAME
/// directory, because a rename across filesystems is not one, and it carries
/// this process's id so two runs publishing at once cannot share one.
pub fn publish_state_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let pending = path.with_extension(format!("new.{}", std::process::id()));
    // THE PENDING FILE CARRIES THE MODE, because the rename is what publishes
    // it: a prune that wrote its replacement at the umask's mode would undo
    // the one the append created the file with.
    let mut pending_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(&pending)?;
    // AND AGAIN AFTER THE OPEN, because `mode` above applies only when the
    // open CREATES the file. The pending path carries this process's own id,
    // so a run interrupted between the open and the rename leaves one for the
    // next run of that pid to REUSE, and a reused inode keeps whatever mode it
    // was made with until this narrows it. Set on the open HANDLE rather than
    // on the path, so nothing can be swapped in underneath between the two.
    pending_file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))?;
    pending_file.write_all(format!("{line}\n").as_bytes())?;
    if let Err(error) = std::fs::rename(&pending, path) {
        // Nothing half-written is left in the state directory for the next
        // run to trip over.
        let _ = std::fs::remove_file(&pending);
        return Err(error);
    }
    Ok(())
}
