use crate::private_directory::PrivateDirectory;
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
        self.config_check_in(&std::env::temp_dir())
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
    fn config_check_in(&mut self, scratch: &std::path::Path) -> Result<(), InspectionFailure> {
        let database =
            PrivateDirectory::create(scratch).map_err(|_| InspectionFailure::Unavailable)?;
        let io = CommandIo::Inspection {
            merge_stderr: false,
        };
        let result = if let Some(daemon) = &self.daemon {
            self.runner
                .run(
                    &self.sudo,
                    &[
                        "-n".as_ref(),
                        daemon.as_os_str(),
                        "--config_path".as_ref(),
                        self.target.join("osquery.conf").as_os_str(),
                        "--config_check".as_ref(),
                        "--database_path".as_ref(),
                        database.path().join("db").as_os_str(),
                    ],
                    io,
                )
                .map(|_| ())
        } else {
            self.command("config-check", io)
        };
        let cleanup =
            std::fs::remove_dir_all(database.path()).map_err(|_| InspectionFailure::Unavailable);
        result.and(cleanup)
    }
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
