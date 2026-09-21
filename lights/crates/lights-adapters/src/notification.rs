use lights_application::Notifier;
use lights_domain::Action;
use std::{
    io,
    os::unix::process::{CommandExt, ExitStatusExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

type RunBounded = fn(&mut Command, Duration) -> io::Result<ExitStatus>;

/// The exit code an overrun reports: 128 plus the signal that ended the group.
const TIMED_OUT: i32 = 128 + libc::SIGKILL;
/// What the group gets between SIGTERM and SIGKILL.
const GRACE: Duration = Duration::from_millis(250);
/// How often the wait looks at the child.
const POLL: Duration = Duration::from_millis(1);

pub struct PnsNotifier<F = RunBounded> {
    pns: PathBuf,
    duration: Duration,
    run: F,
}

impl PnsNotifier {
    pub fn new(home: &Path) -> Self {
        Self::with_runner(home, run_bounded)
    }
}
impl<F: Fn(&mut Command, Duration) -> io::Result<ExitStatus>> PnsNotifier<F> {
    pub fn with_runner(home: &Path, run: F) -> Self {
        Self {
            pns: home.join(".cargo/bin/pns"),
            duration: Duration::from_secs(2),
            run,
        }
    }
}
impl<F: Fn(&mut Command, Duration) -> io::Result<ExitStatus>> Notifier for PnsNotifier<F> {
    fn alarm(&self, detail: &str) {
        self.send(&[
            "--state",
            "failed",
            "--project",
            "hue bridge",
            "--detail",
            detail,
        ]);
    }
    fn announce(&self, action: &Action) {
        let room = match action {
            Action::PowerSet { room, .. }
            | Action::BrightnessSet { room, .. }
            | Action::BrightnessStepped { room, .. }
            | Action::SceneSet { room, .. }
            | Action::Reported { room, .. } => room,
        };
        self.send(&[
            "--state",
            "done",
            "--project",
            room.as_str(),
            "--detail",
            crate::render_action(action).trim_end_matches('\n'),
        ]);
    }
}
impl<F: Fn(&mut Command, Duration) -> io::Result<ExitStatus>> PnsNotifier<F> {
    /// One bounded invocation, whatever it is saying.
    fn send(&self, what: &[&str]) {
        let mut command = Command::new(&self.pns);
        command
            .args(["send", "--producer", "lights"])
            .args(what)
            .args(["--scope", "local_only"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // No status is delivery acknowledgement, and a spawn or wait failure
        // has no unbounded fallback. All outcomes preserve the action.
        let _ = (self.run)(&mut command, self.duration);
    }
}

/// Spawns the child as its own group leader and waits up to `duration`, then
/// ends the group and reports the timeout exit code.
fn run_bounded(command: &mut Command, duration: Duration) -> io::Result<ExitStatus> {
    let mut child = command.process_group(0).spawn()?;
    let deadline = Instant::now() + duration;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                terminate(&mut child);
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            return Ok(ExitStatus::from_raw(TIMED_OUT << 8));
        }
        std::thread::sleep(POLL);
    }
}

/// SIGTERM the group, allow the grace, then SIGKILL whatever is still there.
fn terminate(child: &mut Child) {
    signal_group(child, libc::SIGTERM);
    let grace = Instant::now() + GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < grace => std::thread::sleep(POLL),
            _ => break,
        }
    }
    signal_group(child, libc::SIGKILL);
    let _ = child.wait();
}

/// The unreaped child still owns the process group created at spawn.
fn signal_group(child: &Child, signal: i32) {
    // SAFETY: a signal to the group of a child this process has not reaped.
    unsafe { libc::kill(-(child.id() as i32), signal) };
}

#[cfg(test)]
mod tests;
