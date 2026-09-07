//! The command lane: the PRODUCER API, a generic adapter that runs any
//! executable the block names under the locked contract.
//!
//! THE CONTRACT: a JSON run event on the child's stdin, the exit code as the
//! verdict. Its stdin is PRE-FILLED before the child spawns, rather than
//! written to it after, because uu resets SIGPIPE to SIG_DFL at start-up
//! (main.rs), and a write to a child's stdin after spawn can kill uu with
//! status 141 if that child exits without reading it.

use crate::config::CommandLane;
use crate::lanes::text::stdout_lines;
use crate::lanes::{CommandRunner, LaneAdapter, Verdict};
use uu_domain::LaneReport;
use uu_domain::RunFacts;
use uu_protocol::lane_event;

/// STDOUT IS KEPT EVEN ON A NON-CLEAN EXIT. `run_with_input`'s `Ran::verdict`
/// already carries the reason (the exit description and the stderr tail);
/// what the child printed on the way there is still worth recording, and
/// `report.noted` runs before `report.failed`/`report.deferred`/`report.pending` so it does.
///
/// THE CHILD'S WORLD: `run[0]` is the program, `run[1..]` its arguments, and
/// `argv[0]` the child sees is `run[0]` verbatim. Env and working directory are
/// INHERITED from uu's own process; under the tracked LaunchAgent that is the
/// plist's own PATH plus HOME, with the working directory at `/`. It runs in
/// a PROCESS GROUP OF ITS OWN, bounded by the lane's `deadline_secs`: a child
/// that leaves something behind holding its stdout or stderr (a backgrounded
/// process, a detached daemon) has that group killed at the deadline and the
/// lane reports the overrun as a failure.
impl LaneAdapter for CommandLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_command_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn diagnostic_program(&self) -> Option<&str> {
        Some(&self.run[0])
    }

    fn run(&self, name: &str, facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let program = self.run[0].as_str();
        let args: Vec<&str> = self.run[1..].iter().map(String::as_str).collect();
        let event = lane_event(name, &crate::record::event_for(facts));
        match runner.run_with_input(program, &args, &event) {
            Ok(ran) => {
                for line in stdout_lines(&ran.stdout) {
                    report.noted(line);
                }
                match ran.verdict {
                    Verdict::Clean => {}
                    Verdict::Deferred(reason) => {
                        report.deferred(format!("{program}: deferred ({reason})"));
                    }
                    Verdict::Pending(reason) => {
                        report.pending(format!("{program}: pending ({reason})"));
                    }
                    Verdict::Failed(reason) => report.failed(format!("{program}: {reason}")),
                }
            }
            Err(could_not_run) => report.failed(could_not_run),
        }
        report
    }
}

#[cfg(test)]
mod tests;
