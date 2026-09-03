//! The uv lane: every tool `uv` installed, upgraded in place, weekly.
//!
//! ONE COMMAND AND NO ROSTER. `uv tool upgrade --all` already walks every
//! installed tool, so the whole lane is that call, the line it leaves in the
//! record, and the exit code as the verdict.
//!
//! AN ABSENT `uv` IS A FAILURE HERE, deliberately unlike the bash weekly job
//! this ports from, which printed "nothing to upgrade" and returned clean when
//! the binary was missing. A machine that declares the lane and has no uv is a
//! machine whose tools stopped being upgraded, and a record that reads the same
//! either way is how that goes unnoticed for months.

use crate::config::UvLane;
use crate::lanes::{CommandRunner, LaneReport};

/// The arguments the lane always runs `uv` with.
const UPGRADE: [&str; 3] = ["tool", "upgrade", "--all"];

/// Upgrade every uv tool, and report what that took.
pub fn run_uv(name: &str, lane: &UvLane, runner: &dyn CommandRunner) -> LaneReport {
    let mut report = LaneReport::new(name);
    let binary = lane.binary.as_str();
    match runner.run(binary, &UPGRADE) {
        // uv NARRATES ON STDERR, which a clean run drops, so this line is
        // what the record has to say the lane ran at all.
        Ok(_) => report.noted(format!("{binary} tool upgrade --all: ok")),
        Err(why) => report.failed(format!("{binary} tool upgrade --all FAILED ({why})")),
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Ran;
    use std::cell::RefCell;

    /// A runner that answers one scripted result and records every call it
    /// was asked to make. `Err` is the seam's own contract: an already
    /// composed reason, whether the command exited non-zero or could not be
    /// run at all.
    struct StubRunner {
        answer: Result<String, String>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl StubRunner {
        fn clean() -> Self {
            StubRunner {
                answer: Ok(String::new()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn refusing(why: &str) -> Self {
            StubRunner {
                answer: Err(why.to_string()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }
    }

    impl CommandRunner for StubRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            let mut call = vec![program.to_string()];
            call.extend(args.iter().map(|word| (*word).to_string()));
            self.calls.borrow_mut().push(call);
            self.answer.clone()
        }

        fn run_with_input(
            &self,
            _program: &str,
            _args: &[&str],
            _input: &str,
        ) -> Result<Ran, String> {
            unreachable!("the uv lane hands its child nothing on stdin")
        }
    }

    fn lane() -> UvLane {
        UvLane {
            binary: "/opt/homebrew/bin/uv".to_string(),
        }
    }

    #[test]
    fn the_lane_upgrades_every_uv_tool_with_one_call_to_the_declared_binary() {
        let runner = StubRunner::clean();
        run_uv("uv", &lane(), &runner);
        assert_eq!(
            runner.calls(),
            vec![
                ["/opt/homebrew/bin/uv", "tool", "upgrade", "--all"]
                    .map(String::from)
                    .to_vec()
            ]
        );
    }

    #[test]
    fn a_clean_upgrade_is_one_recorded_line_under_the_lanes_own_name() {
        // THE LANE'S OWN NAME, never the type's: `[lanes.tools]` with
        // `type = "uv"` is recorded and alerted as `tools`, and a report
        // carrying a hardcoded `uv` would name a lane nobody declared.
        let report = run_uv("tools", &lane(), &StubRunner::clean());
        assert_eq!(report.name, "tools");
        assert_eq!(report.failures, 0);
        assert_eq!(report.last_failure, None);
        assert_eq!(
            report.lines,
            vec!["/opt/homebrew/bin/uv tool upgrade --all: ok"]
        );
    }

    #[test]
    fn an_upgrade_that_did_not_succeed_is_a_counted_failure_carrying_what_uv_said() {
        let report = run_uv(
            "uv",
            &lane(),
            &StubRunner::refusing("exit 2: error: no such option `--all`"),
        );
        assert_eq!(report.failures, 1);
        let line = report.last_failure.expect("a failure names itself");
        assert!(line.contains("exit 2: error: no such option"), "{line}");
        assert_eq!(report.lines, vec![line]);
    }

    #[test]
    fn a_uv_that_is_not_installed_is_a_failure_rather_than_a_quiet_skip() {
        // The bash job's own behavior, deliberately not ported: it printed
        // "uv is not at ...; nothing to upgrade" and returned 0.
        let report = run_uv(
            "uv",
            &lane(),
            &StubRunner::refusing("could not run /opt/homebrew/bin/uv: No such file or directory"),
        );
        assert_eq!(report.failures, 1);
        assert!(
            report
                .last_failure
                .is_some_and(|line| line.contains("No such file or directory")),
            "an absent uv must name itself in the record"
        );
    }
}
