use pns_application::{LoopLease, LoopLeases};
use std::path::{Path, PathBuf};

pub struct FileLoopLeases {
    state: PathBuf,
}
impl FileLoopLeases {
    pub fn new(state: PathBuf) -> Self {
        Self { state }
    }
}
impl LoopLeases for FileLoopLeases {
    fn begin(&self, pane: &str, now: u64) -> Result<(), String> {
        let marker = super::lease_marker(&self.state, pane).ok_or("invalid pane")?;
        crate::publish_state_line(&marker, &now.to_string()).map_err(|error| error.to_string())
    }
    fn end(&self, pane: &str) -> Result<(), String> {
        end_lease(&self.state, pane)
    }
}
impl LoopLease for FileLoopLeases {
    fn renew(&self, pane: &str, now: Option<u64>) {
        super::renew_loop_lease(&self.state, pane, now);
    }
}

/// Give a lease back, or say why it could not be given back.
///
/// LOUD, because a human is waiting on the answer and the lamp is a liveness
/// signal: reporting that a loop has ended while its lease is still on disk
/// leaves the loop lamp breathing for the whole timeout with nothing behind it,
/// and the operator has been told the opposite.
///
/// A LEASE THAT IS NOT THERE IS NOT A FAILURE. `pns loop end` on a machine that
/// never began, or a second one after the first, is a removal of a file that is
/// already gone, which is exactly the state the command is for.
pub(super) fn end_lease(state: &Path, pane: &str) -> Result<(), String> {
    let Some(marker) = crate::marker_files::lease_marker(state, pane) else {
        return Ok(());
    };
    match std::fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
#[path = "lease_control/tests.rs"]
mod lease_control_tests;
