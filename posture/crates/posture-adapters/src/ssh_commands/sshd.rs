use super::*;
use posture_application::Sshd;
use std::path::PathBuf;

pub struct SshdCommand<R> {
    runner: R,
    executable: PathBuf,
    main: PathBuf,
    sudo: Option<PathBuf>,
}
impl<R: CommandRunner> SshdCommand<R> {
    pub fn new(runner: R, executable: PathBuf, main: PathBuf, sudo: Option<PathBuf>) -> Self {
        Self {
            runner,
            executable,
            main,
            sudo,
        }
    }
}
impl<R: CommandRunner> Sshd for SshdCommand<R> {
    fn available(&self) -> bool {
        runnable(&self.executable)
    }
    fn global(&mut self) -> SshCommandResult {
        run(
            &mut self.runner,
            None,
            &self.executable,
            &["-G".as_ref(), "-f".as_ref(), self.main.as_os_str()],
            CAPTURE,
        )
    }
    fn connection(&mut self, specification: &str) -> SshCommandResult {
        run(
            &mut self.runner,
            None,
            &self.executable,
            &[
                "-G".as_ref(),
                "-T".as_ref(),
                "-C".as_ref(),
                specification.as_ref(),
                "-f".as_ref(),
                self.main.as_os_str(),
            ],
            CAPTURE,
        )
    }
    fn syntax(&mut self) -> SshCommandResult {
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            &self.executable,
            &["-t".as_ref(), "-f".as_ref(), self.main.as_os_str()],
            CAPTURE,
        )
    }
}
