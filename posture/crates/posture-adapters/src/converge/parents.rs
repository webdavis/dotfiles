use crate::ProcessLookup;
use posture_application::{InspectionFailure, ProcessTable};
use posture_domain::ParentPid;

/// The daemon's own parent, read out of the process table.
pub struct OsqueryParents<P>(pub(super) P);
impl<P: ProcessLookup> OsqueryParents<P> {
    pub fn new(processes: P) -> Self {
        Self(processes)
    }
}
impl<P: ProcessLookup> ProcessTable for OsqueryParents<P> {
    fn daemon_parent(&mut self) -> Result<Option<ParentPid>, InspectionFailure> {
        let pids = self.0.matching(DAEMON, None, Some(LAUNCHD))?;
        // THE LOWEST MATCHING ID, so a machine that somehow holds two
        // launchd-parented daemons is judged on one of them rather than on
        // whichever the kernel happened to list first.
        let Some(pid) = pids.first() else {
            return Ok(None);
        };
        ParentPid::parse(&pid.to_string())
            .map(Some)
            .ok_or(InspectionFailure::Failed)
    }
}

const DAEMON: &str = "osqueryd";
/// `launchd`, the parent a supervised daemon must have.
const LAUNCHD: u32 = 1;

#[cfg(test)]
mod tests;
