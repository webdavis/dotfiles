use crate::{CommandIo, CommandRunner};
use posture_application::{InspectionFailure, OsqueryControl, VendorPlist};
use std::path::PathBuf;

pub struct OsqueryRestart<R> {
    pub(super) runner: R,
    sudo: PathBuf,
    command: PathBuf,
    target: PathBuf,
}
impl<R: CommandRunner> OsqueryRestart<R> {
    pub fn new(runner: R, sudo: PathBuf, command: PathBuf, target: PathBuf) -> Self {
        Self {
            runner,
            sudo,
            command,
            target,
        }
    }
}
impl<R: CommandRunner> OsqueryControl for OsqueryRestart<R> {
    fn vendor_plist(&mut self) -> VendorPlist {
        match std::fs::symlink_metadata(self.target.join("io.osquery.agent.plist")) {
            Ok(metadata) if metadata.file_type().is_symlink() => VendorPlist::Symlink,
            Ok(metadata) if metadata.is_file() => VendorPlist::Regular,
            _ => VendorPlist::Missing,
        }
    }
    fn config_check(&mut self) -> Result<(), InspectionFailure> {
        self.command(
            "config-check",
            CommandIo::Inspection {
                merge_stderr: false,
            },
        )
    }
    fn stop(&mut self) -> Result<(), InspectionFailure> {
        self.command(
            "stop",
            CommandIo::Inspection {
                merge_stderr: false,
            },
        )
    }
    fn start(&mut self) -> Result<(), InspectionFailure> {
        self.command("start", CommandIo::InheritAll)
    }
}
impl<R: CommandRunner> OsqueryRestart<R> {
    fn command(&mut self, verb: &str, io: CommandIo<'_>) -> Result<(), InspectionFailure> {
        self.runner
            .run(
                &self.sudo,
                &["-n".as_ref(), self.command.as_os_str(), verb.as_ref()],
                io,
            )
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests;
