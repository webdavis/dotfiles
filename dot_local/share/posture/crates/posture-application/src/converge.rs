use posture_domain::{
    ContentComparison, ConvergeDirectory, ConvergeFile, Drift, LiveEntry, directory_drift,
    file_drift, restart_required,
};
use std::path::{Path, PathBuf};

mod repair;
mod restart;
pub use repair::{ConvergeEvent, ConvergeFailure, PrivilegedInstall, converge};
pub use restart::{
    OsqueryControl, ProcessTable, RestartClock, RestartFailure, Restarted, VendorPlist,
    restart_daemon,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagingRefusal {
    RelativeDirectory(PathBuf),
    SymlinkComponent(PathBuf),
    MissingDirectory(PathBuf),
    IncompleteListing(PathBuf),
    SymlinkEntry(PathBuf),
    UnlistedEntry(PathBuf),
    MissingFile(PathBuf),
    CopyFailed(PathBuf),
    PrivateDirectory(PathBuf),
}

// The prepared value owns the private copy until its consumer finishes reading it.
// No live target or privileged operation belongs to this preparation boundary.
pub trait ConvergeStaging {
    type Prepared: DesiredTree;

    fn prepare(&self) -> Result<Self::Prepared, Vec<StagingRefusal>>;
}

pub trait DesiredTree {
    fn source(&self, file: ConvergeFile) -> PathBuf;
}

pub trait LiveTree {
    fn directory(&mut self, directory: ConvergeDirectory) -> LiveEntry;
    fn file(&mut self, file: ConvergeFile, desired: &Path) -> (LiveEntry, ContentComparison);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvergeRefusal {
    Staging(Vec<StagingRefusal>),
    IrregularDirectory(ConvergeDirectory),
}

#[derive(Debug)]
pub struct ConvergePlan<P> {
    pub desired: P,
    pub directories: [(ConvergeDirectory, Drift); 2],
    pub files: [(ConvergeFile, Drift); 6],
}

impl<P> ConvergePlan<P> {
    pub fn needs_restart(&self) -> bool {
        restart_required(&self.directories.map(|(_, verdict)| verdict))
            || restart_required(&self.files.map(|(_, verdict)| verdict))
    }
}

pub fn prepare_converge<S: ConvergeStaging>(
    staging: &S,
    live: &mut impl LiveTree,
) -> Result<ConvergePlan<S::Prepared>, ConvergeRefusal> {
    let desired = staging.prepare().map_err(ConvergeRefusal::Staging)?;
    let directories = directory_verdicts(live)?;
    let files = ConvergeFile::ALL.map(|file| {
        let (entry, content) = live.file(file, &desired.source(file));
        (file, file_drift(entry, content))
    });
    Ok(ConvergePlan {
        desired,
        directories,
        files,
    })
}

fn directory_verdicts(
    live: &mut impl LiveTree,
) -> Result<[(ConvergeDirectory, Drift); 2], ConvergeRefusal> {
    let directories = ConvergeDirectory::ALL
        .map(|directory| (directory, directory_drift(live.directory(directory))));
    // Inspect both directories before allowing either one into a repair plan (S326).
    for (directory, verdict) in directories {
        if verdict == Drift::Irregular {
            return Err(ConvergeRefusal::IrregularDirectory(directory));
        }
    }
    Ok(directories)
}

#[cfg(test)]
mod tests;
