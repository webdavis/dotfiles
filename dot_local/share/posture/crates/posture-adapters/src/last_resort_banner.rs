use crate::{CommandIo, CommandRunner};
use posture_application::{AlarmFailed, IndependentAlarm};
use std::{ffi::OsStr, path::PathBuf};

pub struct LastResortBanner<R> {
    runner: R,
    executable: PathBuf,
}
impl<R: CommandRunner> LastResortBanner<R> {
    pub fn new(runner: R, executable: PathBuf) -> Self {
        Self { runner, executable }
    }
}
impl<R: CommandRunner> IndependentAlarm for LastResortBanner<R> {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed> {
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"Sosumi\"",
            literal(detail),
            literal(title)
        );
        self.runner
            .run(
                &self.executable,
                &[OsStr::new("-e"), OsStr::new(&script)],
                CommandIo::Inspection {
                    merge_stderr: false,
                },
            )
            .map(|_| ())
            .map_err(|_| AlarmFailed)
    }
}
fn literal(text: &str) -> String {
    // Backslash first: an existing escape must not turn a later quote into executable source.
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
#[cfg(test)]
mod tests;
