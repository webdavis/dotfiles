use crate::{CommandIo, CommandRunner};
use posture_application::{InspectionFailure, OsqueryControl, VendorPlist};
use std::path::PathBuf;

pub struct OsqueryRestart<R> {
    pub(super) runner: R,
    sudo: PathBuf,
    command: PathBuf,
    daemon: Option<PathBuf>,
    target: PathBuf,
}
impl<R: CommandRunner> OsqueryRestart<R> {
    pub fn new(
        runner: R,
        sudo: PathBuf,
        command: PathBuf,
        daemon: Option<PathBuf>,
        target: PathBuf,
    ) -> Self {
        Self {
            runner,
            sudo,
            command,
            daemon,
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
        let io = CommandIo::Inspection {
            merge_stderr: false,
        };
        // Measured against osqueryi 5.23.1: --disable_database opens no database at
        // all, so the check neither contends with the lock the running daemon holds
        // on the live one nor creates a database of its own for the unprivileged
        // converge to clean up after root. osqueryctl runs its own check, and picks
        // its own database path, so the fallback needs nothing from this caller.
        let Some(daemon) = &self.daemon else {
            return self.command("config-check", io);
        };
        self.runner
            .run(
                &self.sudo,
                &[
                    "-n".as_ref(),
                    daemon.as_os_str(),
                    "--config_path".as_ref(),
                    self.target.join("osquery.conf").as_os_str(),
                    "--config_check".as_ref(),
                    "--disable_database".as_ref(),
                ],
                io,
            )
            .map(|_| ())
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

#[cfg(test)]
mod validation_tests;
