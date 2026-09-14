use super::*;
use posture_application::SshLaunchctl;
use std::path::PathBuf;

pub struct SshLaunchd<R> {
    runner: R,
    executable: PathBuf,
    sudo: Option<PathBuf>,
}
impl<R: CommandRunner> SshLaunchd<R> {
    pub fn new(runner: R, executable: PathBuf, sudo: Option<PathBuf>) -> Self {
        Self {
            runner,
            executable,
            sudo,
        }
    }
}
impl<R: CommandRunner> SshLaunchctl for SshLaunchd<R> {
    fn probe(&mut self) -> SshCommandResult {
        run(
            &mut self.runner,
            None,
            &self.executable,
            &["print".as_ref(), "system/com.openssh.sshd".as_ref()],
            CAPTURE,
        )
    }
    fn restart(&mut self) -> SshCommandResult {
        run(
            &mut self.runner,
            self.sudo.as_deref(),
            &self.executable,
            &[
                "kickstart".as_ref(),
                "-k".as_ref(),
                "system/com.openssh.sshd".as_ref(),
            ],
            CAPTURE,
        )
    }
}
