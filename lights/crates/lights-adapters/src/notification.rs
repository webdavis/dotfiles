use lights_application::Notifier;
use lights_domain::Action;
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::Duration,
};

type RunMonitor = fn(&mut Command) -> io::Result<ExitStatus>;

pub struct PnsNotifier<F = RunMonitor> {
    pns: PathBuf,
    monitor: PathBuf,
    duration: Duration,
    run: F,
}

impl PnsNotifier {
    pub fn new(home: &Path) -> Self {
        Self::with_runner(home, run_monitor)
    }
}
impl<F: Fn(&mut Command) -> io::Result<ExitStatus>> PnsNotifier<F> {
    pub fn with_runner(home: &Path, run: F) -> Self {
        Self {
            pns: home.join(".local/libexec/pns/pns"),
            monitor: PathBuf::from("gtimeout"),
            duration: Duration::from_secs(2),
            run,
        }
    }
}
impl<F: Fn(&mut Command) -> io::Result<ExitStatus>> Notifier for PnsNotifier<F> {
    fn announce(&self, action: &Action) {
        let room = match action {
            Action::PowerSet { room, .. }
            | Action::BrightnessSet { room, .. }
            | Action::BrightnessStepped { room, .. }
            | Action::SceneSet { room, .. }
            | Action::Reported { room, .. } => room,
        };
        let mut command = Command::new(&self.monitor);
        command
            .args(["--foreground", "--signal=KILL"])
            .arg(format!("{}s", self.duration.as_secs_f64()))
            .arg(&self.pns)
            .args([
                "--agent",
                "lights",
                "--state",
                "done",
                "--project",
                room.as_str(),
                "--detail",
                crate::render_action(action).trim_end_matches('\n'),
                "--local-only",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // No status is delivery acknowledgement. All outcomes preserve the action.
        let status = match (self.run)(&mut command) {
            Ok(status) => status,
            Err(_) => return, // Spawn or wait failure, with no unbounded fallback.
        };
        match status.code() {
            Some(0) => (),         // The invocation finished.
            Some(125) => (),       // Monitor failure (or the child's same exit code).
            Some(126 | 127) => (), // Unable to run, or the child's same exit code.
            Some(137) => (),       // Timeout, signal kill, or the child's same exit code.
            Some(_) | None => (),
        }
    }
}
fn run_monitor(command: &mut Command) -> io::Result<ExitStatus> {
    let mut child = command.spawn()?;
    match child.wait() {
        Ok(status) => Ok(status),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
