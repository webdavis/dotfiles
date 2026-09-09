use super::*;

/// Where jobs are spooled, one file per job named by its id.
pub fn spool_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon")
}

/// Where the markers that cancel jobs live. A job carries a marker NAME, never
/// a path, and it is resolved here, so the field cannot become a general
/// filesystem probe.
pub fn marker_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon-markers")
}

/// Where the daemon says it is alive.
///
/// BESIDE THE SPOOL AND NOT INSIDE IT: a heartbeat file in the spool directory
/// would be read as a job every tick, refused as unparseable and dropped, so
/// the daemon would spend its life deleting its own pulse.
pub fn heartbeat_path(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon-heartbeat")
}

/// The mode every file this module writes carries, matching every other state
/// file the crate publishes.
pub(super) const STATE_FILE_MODE: u32 = 0o600;

/// The prefix this module's own working files carry, and which no valid id can
/// start with.
///
/// `~` IS OUTSIDE THE ID CHARSET, which is what makes this a rule rather than a
/// convention: a claim and a pending write both live in the spool directory,
/// and the scan has to be able to tell them from a job without parsing them.
pub(super) const WORKING_PREFIX: &str = "~";
