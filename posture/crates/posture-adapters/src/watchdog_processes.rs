use crate::{CommandIo, CommandRunner};
use posture_application::{DaemonHealth, WatchdogProcesses};
use posture_domain::{Agent, AgentExit, AgentReading};
use std::{ffi::OsStr, path::Path};

pub struct SystemWatchdogProcesses<R> {
    runner: R,
    uid: u32,
    output: String,
}
impl<R: CommandRunner> SystemWatchdogProcesses<R> {
    pub fn new(runner: R, uid: u32) -> Self {
        Self {
            runner,
            uid,
            output: String::new(),
        }
    }
    pub fn current_user(runner: R) -> Self {
        // getuid takes no pointers and always returns the calling process identity.
        Self::new(runner, unsafe { libc::getuid() })
    }
    fn print(&mut self, label: &str) -> bool {
        let target = format!("gui/{}/{label}", self.uid);
        match self.runner.run(
            Path::new("/bin/launchctl"),
            &[OsStr::new("print"), OsStr::new(&target)],
            CommandIo::Inspection {
                merge_stderr: false,
            },
        ) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(output) => {
                    self.output = output;
                    true
                }
                Err(_) => {
                    self.output.clear();
                    true
                }
            },
            Err(_) => {
                self.output.clear();
                false
            }
        }
    }
}
impl<R: CommandRunner> WatchdogProcesses for SystemWatchdogProcesses<R> {
    fn osquery_running(&mut self) -> bool {
        self.runner
            .run(
                Path::new("/usr/bin/pgrep"),
                &[OsStr::new("-fq"), OsStr::new("/opt/osquery/.*osqueryd")],
                CommandIo::Inspection {
                    merge_stderr: false,
                },
            )
            .is_ok()
    }
    fn agent(&mut self, agent: Agent) -> AgentReading<'_> {
        if !self.print(agent.label()) {
            return AgentReading::Unloaded;
        }
        AgentReading::Loaded {
            runs: field(&self.output, "runs").and_then(decimal),
            exit: AgentExit::from_field(field(&self.output, "last exit code")),
        }
    }
    fn pns_daemon(&mut self) -> DaemonHealth {
        if !self.print("com.webdavis.pns-daemon") {
            return DaemonHealth::Unloaded;
        }
        let pid = field(&self.output, "pid")
            .and_then(decimal)
            .filter(|pid| (1..=i32::MAX as u64).contains(pid));
        let Some(pid) = pid.filter(|_| field(&self.output, "state") == Some("running")) else {
            return DaemonHealth::NotRunning;
        };
        if self
            .runner
            .run(
                Path::new("/bin/kill"),
                &[OsStr::new("-0"), OsStr::new(&pid.to_string())],
                CommandIo::Inspection {
                    merge_stderr: false,
                },
            )
            .is_ok()
        {
            DaemonHealth::Running
        } else {
            DaemonHealth::NotRunning
        }
    }
}
fn field<'a>(output: &'a str, name: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let (key, value) = line.trim().split_once(" = ")?;
        (key == name).then_some(value.trim())
    })
}
fn decimal(value: &str) -> Option<u64> {
    (!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}
#[cfg(test)]
mod tests;
