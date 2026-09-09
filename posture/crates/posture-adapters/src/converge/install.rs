use crate::{CommandIo, CommandRunner};
use posture_application::{InspectionFailure, PrivilegedInstall};
use posture_domain::{ConvergeDirectory, ConvergeFile};
use std::path::{Path, PathBuf};

pub struct ConvergeInstaller<R> {
    pub(super) runner: R,
    sudo: PathBuf,
    target: PathBuf,
    log: PathBuf,
}
impl<R: CommandRunner> ConvergeInstaller<R> {
    pub fn new(runner: R, sudo: PathBuf, target: PathBuf, log: PathBuf) -> Self {
        Self {
            runner,
            sudo,
            target,
            log,
        }
    }
}
impl<R: CommandRunner> PrivilegedInstall for ConvergeInstaller<R> {
    fn directory(&mut self, directory: ConvergeDirectory) -> Result<(), InspectionFailure> {
        let target = match directory {
            ConvergeDirectory::Target => self.target.clone(),
            ConvergeDirectory::Packs => self.target.join("packs"),
        };
        self.runner
            .run(
                &self.sudo,
                &[
                    "-n".as_ref(),
                    "/usr/bin/install".as_ref(),
                    "-d".as_ref(),
                    "-o".as_ref(),
                    "root".as_ref(),
                    "-g".as_ref(),
                    "wheel".as_ref(),
                    "-m".as_ref(),
                    "0755".as_ref(),
                    target.as_os_str(),
                ],
                CommandIo::InheritAll,
            )
            .map(|_| ())
    }
    fn file(&mut self, file: ConvergeFile, source: &Path) -> Result<(), InspectionFailure> {
        let target = self.target.join(file.relative_path());
        self.runner
            .run(
                &self.sudo,
                &[
                    "-n".as_ref(),
                    "/usr/bin/install".as_ref(),
                    "-o".as_ref(),
                    "root".as_ref(),
                    "-g".as_ref(),
                    "wheel".as_ref(),
                    "-m".as_ref(),
                    "0644".as_ref(),
                    source.as_os_str(),
                    target.as_os_str(),
                ],
                CommandIo::InheritAll,
            )
            .map(|_| ())
    }
    fn log_directory(&mut self) -> Result<bool, InspectionFailure> {
        if self.log.is_dir() {
            return Ok(false);
        }
        std::fs::create_dir_all(&self.log).map_err(|_| InspectionFailure::Failed)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
