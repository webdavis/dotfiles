//! The daemon's own launchd service, wrapped the way `hermes gateway` wraps
//! its own. `pns gateway start|stop|restart|status` is the only caller.

use pns_application::{ServiceController, ServiceError, ServiceState};
use std::process::Command;

/// One launchctl call's answer: whether it exited zero, and both streams.
pub struct LaunchctlOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// What runs one launchctl call, injected so the adapter's argv-building and
/// output-parsing are tested without touching real launchd.
pub trait LaunchctlRunner {
    fn run(&self, args: &[&str]) -> LaunchctlOutput;
}

/// The production runner. NO DEADLINE, unlike `SystemCommandRunner`: this
/// sits behind an operator-typed command rather than a notification path, so
/// a hang is something the operator watching the terminal can interrupt
/// rather than a probe silently starving an event nobody is looking at.
pub struct SystemLaunchctlRunner;

impl LaunchctlRunner for SystemLaunchctlRunner {
    fn run(&self, args: &[&str]) -> LaunchctlOutput {
        match Command::new("launchctl").args(args).output() {
            Ok(output) => LaunchctlOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
            Err(error) => LaunchctlOutput {
                success: false,
                stdout: String::new(),
                stderr: error.to_string(),
            },
        }
    }
}

/// `launchctl bootstrap`'s own text for a service already loaded: an EIO
/// wrapped in a sentence, not a name any launchctl subcommand spells
/// consistently. Measured live against a running LaunchAgent on macOS 26.2.
const ALREADY_LOADED: &str = "Input/output error";

/// `launchctl kickstart` and `launchctl print`'s own text for a label nothing
/// has bootstrapped. Measured live on macOS 26.2.
const NOT_BOOTSTRAPPED: &str = "Could not find service";

/// `launchctl bootout`'s own text for a label nothing has bootstrapped.
/// Measured live on macOS 26.2.
const NOT_BOOTED_IN: &str = "No such process";

/// Builds `launchctl`'s argv from a label and a home directory, and reads its
/// stdout, stderr and exit back into the port's vocabulary.
pub struct LaunchdServiceController<R: LaunchctlRunner> {
    pub runner: R,
    pub home: String,
}

impl<R: LaunchctlRunner> LaunchdServiceController<R> {
    fn uid(&self) -> u32 {
        // SAFETY: takes no argument and cannot fail.
        unsafe { libc::getuid() }
    }

    fn domain_target(&self) -> String {
        format!("gui/{}", self.uid())
    }

    fn service_target(&self, label: &str) -> String {
        format!("{}/{label}", self.domain_target())
    }

    fn plist_path(&self, label: &str) -> String {
        format!("{}/Library/LaunchAgents/{label}.plist", self.home)
    }
}

impl<R: LaunchctlRunner> ServiceController for LaunchdServiceController<R> {
    /// `bootstrap`, with the ALREADY-LOADED case folded in here rather than
    /// left for the caller: launchctl answers it with a plain `kickstart`
    /// (no `-k`), which is a different argv than the port's own `restart`
    /// runs, so it is this adapter's fact to hold rather than a second verb
    /// the command layer would have to know to call.
    fn start(&self, label: &str) -> Result<(), ServiceError> {
        let plist = self.plist_path(label);
        if !std::path::Path::new(&plist).is_file() {
            return Err(ServiceError::MissingPlist(plist));
        }
        let output = self
            .runner
            .run(&["bootstrap", &self.domain_target(), &plist]);
        if output.success {
            return Ok(());
        }
        if output.stderr.contains(ALREADY_LOADED) {
            let retry = self.runner.run(&["kickstart", &self.service_target(label)]);
            return if retry.success {
                Ok(())
            } else {
                Err(ServiceError::Failed(retry.stderr.trim().to_string()))
            };
        }
        Err(ServiceError::Failed(output.stderr.trim().to_string()))
    }

    fn stop(&self, label: &str) -> Result<(), ServiceError> {
        let output = self.runner.run(&["bootout", &self.service_target(label)]);
        if output.success {
            return Ok(());
        }
        if output.stderr.contains(NOT_BOOTED_IN) {
            return Err(ServiceError::NotLoaded);
        }
        Err(ServiceError::Failed(output.stderr.trim().to_string()))
    }

    fn restart(&self, label: &str) -> Result<(), ServiceError> {
        let output = self
            .runner
            .run(&["kickstart", "-k", &self.service_target(label)]);
        if output.success {
            return Ok(());
        }
        if output.stderr.contains(NOT_BOOTSTRAPPED) {
            return Err(ServiceError::NotLoaded);
        }
        Err(ServiceError::Failed(output.stderr.trim().to_string()))
    }

    fn status(&self, label: &str) -> Result<ServiceState, ServiceError> {
        let output = self.runner.run(&["print", &self.service_target(label)]);
        if !output.success {
            return if output.stderr.contains(NOT_BOOTSTRAPPED) {
                Ok(ServiceState::NotLoaded)
            } else {
                Err(ServiceError::Failed(output.stderr.trim().to_string()))
            };
        }
        Ok(parse_status(&output.stdout))
    }
}

/// `launchctl print`'s own `state = <word>` and `pid = <n>` lines, read off
/// the FIRST exact match: a `job state` line and every nested endpoint's own
/// `state` sit deeper in the same listing and would otherwise collide with
/// the top-level reading this wants.
fn parse_status(stdout: &str) -> ServiceState {
    let running = stdout.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "state").then(|| value.trim() == "running")
    });
    if running != Some(true) {
        return ServiceState::Loaded;
    }
    let pid = stdout.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key.trim() != "pid" {
            return None;
        }
        value.trim().parse::<u32>().ok()
    });
    match pid {
        Some(pid) => ServiceState::Running { pid },
        // Reported running with no readable pid: the service exists, which
        // is the part this can still stand behind.
        None => ServiceState::Loaded,
    }
}

#[cfg(test)]
#[path = "launchd/tests.rs"]
mod tests;
