use crate::STATE_FILE_MODE;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
/// Where one answered marker lives. The daemon owns the directory and resolves
/// the NAME inside it; this is the same resolution for the two writers that are
/// not the daemon.
pub fn marker_path(state: &Path, marker: &str) -> std::path::PathBuf {
    crate::job_spool::marker_dir(state).join(marker)
}

/// One answered marker written: empty, 0600, and present is the whole message.
pub fn write_marker(state: &Path, marker: &str) -> std::io::Result<()> {
    let path = marker_path(state, marker);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(&path)?;
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: `mode`
    // applies only when the open CREATES the file, and a marker left by an
    // earlier arm in this session is reused rather than made.
    file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))
}
