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
        let io = CommandIo::Inspection {
            merge_stderr: false,
        };
        // osqueryctl picks its own database path, so the fallback needs no private
        // directory and must not inherit a refusal to create one.
        let Some(daemon) = self.daemon.clone() else {
            return self.command("config-check", io);
        };
        let database =
            PrivateDirectory::create(scratch).map_err(|_| InspectionFailure::Unavailable)?;
        // The validation runs under sudo, so osqueryi creates whatever is missing as
        // root: this caller creates the database directory itself, at its own
        // ownership, because the unprivileged converge can remove the flat files
        // root writes inside a directory the caller owns but cannot even list one
        // root created. Removal is the private directory's own drop, and is
        // deliberately not part of the verdict: a database left behind is a
        // temp-directory leak, never a failed configuration check.
        let db = database.path().join("db");
        std::fs::create_dir(&db).map_err(|_| InspectionFailure::Unavailable)?;
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
                    db.as_os_str(),
                ],
                io,
            )
            .map(|_| ())
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
