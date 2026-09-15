use super::*;
use posture_application::{SshFile, SshInstallFiles};
use std::{io, path::PathBuf};

pub struct SshFileInstaller<R> {
    runner: R,
    directory: PathBuf,
    sudo: Option<PathBuf>,
}
impl<R: CommandRunner> SshFileInstaller<R> {
    pub fn new(runner: R, directory: PathBuf, sudo: Option<PathBuf>) -> Self {
        Self {
            runner,
            directory,
            sudo,
        }
    }
}
impl<R: CommandRunner> SshInstallFiles for SshFileInstaller<R> {
    fn path(&self, file: SshFile) -> PathBuf {
        self.directory.join(file.name())
    }
    fn directory_exists(&self) -> bool {
        self.directory.is_dir()
    }
    fn exists(&self, file: SshFile) -> Result<bool, InspectionFailure> {
        match std::fs::symlink_metadata(self.path(file)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(InspectionFailure::Unavailable),
        }
    }
    fn prime(&mut self) -> SshCommandResult {
        match self.sudo.as_deref() {
            Some(sudo) => run(
                &mut self.runner,
                None,
                sudo,
                &["-v".as_ref()],
                CommandIo::InheritAll,
            ),
            None => Ok(SshCompleted {
                status: 0,
                output: Vec::new(),
            }),
        }
    }
    fn stage(&mut self, bytes: &[u8]) -> SshCommandResult {
        let path = self.path(SshFile::Staging);
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            Path::new("/usr/bin/tee"),
            &["--".as_ref(), path.as_os_str()],
            CommandIo::Input(bytes),
        )
    }
    fn chmod(&mut self) -> SshCommandResult {
        let path = self.path(SshFile::Staging);
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            Path::new("/bin/chmod"),
            &["--".as_ref(), "0644".as_ref(), path.as_os_str()],
            CAPTURE,
        )
    }
    fn save(&mut self) -> SshCommandResult {
        let from = self.path(SshFile::Target);
        let to = self.path(SshFile::SavedTarget);
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            Path::new("/bin/cp"),
            &[
                "-Rp".as_ref(),
                "--".as_ref(),
                from.as_os_str(),
                to.as_os_str(),
            ],
            CAPTURE,
        )
    }
    fn rename(&mut self, from: SshFile, to: SshFile) -> SshCommandResult {
        let from = self.path(from);
        let to = self.path(to);
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            Path::new("/bin/mv"),
            &[
                "-f".as_ref(),
                "--".as_ref(),
                from.as_os_str(),
                to.as_os_str(),
            ],
            CAPTURE,
        )
    }
    fn remove(&mut self, files: &[SshFile]) -> SshCommandResult {
        let paths: Vec<_> = files.iter().map(|file| self.path(*file)).collect();
        let mut args = vec![OsStr::new("-f"), OsStr::new("--")];
        args.extend(paths.iter().map(|path| path.as_os_str()));
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            Path::new("/bin/rm"),
            &args,
            CAPTURE,
        )
    }
}
