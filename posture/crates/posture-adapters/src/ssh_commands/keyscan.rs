use super::*;
use posture_application::SshBannerProbe;
use std::path::PathBuf;

pub struct SshKeyscan<R> {
    runner: R,
    executable: PathBuf,
}
impl<R: CommandRunner> SshKeyscan<R> {
    pub fn new(runner: R, executable: PathBuf) -> Self {
        Self { runner, executable }
    }
}
impl<R: CommandRunner> SshBannerProbe for SshKeyscan<R> {
    fn available(&self) -> bool {
        runnable(&self.executable)
    }
    fn probe(&mut self, port: u16, timeout: u32) -> SshCommandResult {
        run(
            &mut self.runner,
            None,
            &self.executable,
            &[
                "-T".as_ref(),
                timeout.to_string().as_ref(),
                "-p".as_ref(),
                port.to_string().as_ref(),
                "127.0.0.1".as_ref(),
            ],
            CommandIo::Inspection {
                merge_stderr: false,
            },
        )
    }
}
