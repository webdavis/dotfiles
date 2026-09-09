use super::{ConvergeRefusal, ConvergeStaging, DesiredTree, LiveTree, directory_verdicts};
use crate::{InspectionFailure, RestartFailure, Restarted};
use posture_domain::{ConvergeDirectory, ConvergeFile, Drift, file_drift};
use std::{io, path::Path};

pub trait PrivilegedInstall {
    fn directory(&mut self, directory: ConvergeDirectory) -> Result<(), InspectionFailure>;
    fn file(&mut self, file: ConvergeFile, source: &Path) -> Result<(), InspectionFailure>;
    fn log_directory(&mut self) -> Result<bool, InspectionFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvergeEvent {
    DirectoryRepaired(ConvergeDirectory, Drift),
    FileInstalled(ConvergeFile, Drift),
    LogDirectoryCreated,
    Restarted(Restarted),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvergeFailure {
    Preparation(ConvergeRefusal),
    Directory(ConvergeDirectory, InspectionFailure),
    File(ConvergeFile, InspectionFailure),
    LogDirectory(InspectionFailure),
    Restart(RestartFailure),
    Report,
}

pub fn converge<S: ConvergeStaging>(
    staging: &S,
    live: &mut impl LiveTree,
    install: &mut impl PrivilegedInstall,
    mut restart: impl FnMut() -> Result<Restarted, RestartFailure>,
    mut report: impl FnMut(ConvergeEvent) -> io::Result<()>,
) -> Result<(), ConvergeFailure> {
    let desired = staging
        .prepare()
        .map_err(|errors| ConvergeFailure::Preparation(ConvergeRefusal::Staging(errors)))?;
    let directories = directory_verdicts(live).map_err(ConvergeFailure::Preparation)?;
    let mut changed = false;
    for (directory, verdict) in directories {
        if verdict == Drift::Ok {
            continue;
        }
        install
            .directory(directory)
            .map_err(|error| ConvergeFailure::Directory(directory, error))?;
        report(ConvergeEvent::DirectoryRepaired(directory, verdict))
            .map_err(|_| ConvergeFailure::Report)?;
        changed = true;
    }
    // Directory repairs can change reachability. Read each live file afterward,
    // against the same private source that the privileged install will read.
    for file in ConvergeFile::ALL {
        let source = desired.source(file);
        let (entry, content) = live.file(file, &source);
        let verdict = file_drift(entry, content);
        if verdict == Drift::Ok {
            continue;
        }
        install
            .file(file, &source)
            .map_err(|error| ConvergeFailure::File(file, error))?;
        report(ConvergeEvent::FileInstalled(file, verdict)).map_err(|_| ConvergeFailure::Report)?;
        changed = true;
    }
    if install
        .log_directory()
        .map_err(ConvergeFailure::LogDirectory)?
    {
        report(ConvergeEvent::LogDirectoryCreated).map_err(|_| ConvergeFailure::Report)?;
    }
    if changed {
        let restarted = restart().map_err(ConvergeFailure::Restart)?;
        report(ConvergeEvent::Restarted(restarted)).map_err(|_| ConvergeFailure::Report)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
