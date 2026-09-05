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
use crate::lanes::{CommandRunner, LaneAdapter, LaneReport, Verdict};
use crate::record::{RunFacts, lane_event};

/// STDOUT IS KEPT EVEN ON A NON-CLEAN EXIT. `run_with_input`'s `Ran::verdict`
/// already carries the reason (the exit description and the stderr tail);
/// what the child printed on the way there is still worth recording, and
/// `report.noted` runs before `report.failed`/`report.deferred` so it does.
///
/// THE CHILD'S WORLD: `run[0]` is the program, `run[1..]` its arguments, and
/// argv[0] the child sees is `run[0]` verbatim. Env and working directory are
/// INHERITED from uu's own process; under the tracked LaunchAgent that is the
/// plist's own PATH plus HOME, with the working directory at `/`. It runs in
/// a PROCESS GROUP OF ITS OWN, bounded by the lane's `deadline_secs`: a child
/// that leaves something behind holding its stdout or stderr (a backgrounded
/// process, a detached daemon) has that group killed at the deadline and the
/// lane reports the overrun as a failure.
impl LaneAdapter for CommandLane {
    fn run(&self, name: &str, facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let program = self.run[0].as_str();
        let args: Vec<&str> = self.run[1..].iter().map(String::as_str).collect();
        let event = lane_event(name, facts);
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
                    Verdict::Failed(reason) => report.failed(format!("{program}: {reason}")),
                }
            }
            Err(could_not_run) => report.failed(could_not_run),
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::stubs::{ScriptedRunner, stub_facts};
    use crate::lanes::text::STDOUT_LINES_KEPT;

    fn command_lane(run: &[&str]) -> CommandLane {
        CommandLane {
            run: run.iter().map(|word| word.to_string()).collect(),
        }
    }

    // --- the command lane -------------------------------------------------------

    #[test]
    fn a_command_lane_hands_the_run_event_to_the_child_on_stdin_and_records_what_it_printed() {
        let runner = ScriptedRunner::new(&[]).answering("3 upgraded\n");
        // Two arguments, so a mutant that reverses run[1..] or drops the
        // second one changes what the runner recorded.
        let lane = command_lane(&["/usr/local/bin/updater", "--yes", "--now"]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            runner.calls(),
            vec![vec![
                "/usr/local/bin/updater".to_string(),
                "--yes".to_string(),
                "--now".to_string(),
            ]],
            "the program is run[0], its arguments are run[1..], in order"
        );
        let inputs = runner.inputs();
        let input = &inputs[0];
        assert!(input.ends_with('\n'), "{input:?}");
        let event: serde_json::Value =
            serde_json::from_str(input.trim_end()).expect("the event is JSON");
        // The COMPLETE parsed event against the same facts `lane_event`
        // itself would produce, not just one field: a mutant that swaps the
        // event for `{"lane":"mine"}` still has a correct `lane` field.
        let recorded: serde_json::Value =
            serde_json::from_str(lane_event("mine", &stub_facts()).trim_end())
                .expect("the reference event is JSON");
        assert_eq!(event, recorded);
        assert!(
            report.lines.contains(&"3 upgraded".to_string()),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_child_that_exits_non_zero_is_a_failure_the_alert_summary_names() {
        let program = "/usr/local/bin/updater";
        let runner = ScriptedRunner::new(&[&[program]]).answering("did some work\n");
        let lane = command_lane(&[program]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report.lines.contains(&"did some work".to_string()),
            "a failed child's stdout is not lost: {:?}",
            report.lines
        );
        let summary = crate::alert::alert_summary(&report);
        assert!(summary.contains("exit 1"), "{summary}");
        assert!(summary.contains(program), "{summary}");
        // THE VERDICT COMES LAST: what the child printed is noted first, so
        // the record reads as the work and then how it ended.
        assert_eq!(
            report.lines.last(),
            report.last_failure.as_ref(),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_child_that_exits_the_deferred_code_is_recorded_deferred_not_failed() {
        let program = "/usr/local/bin/updater";
        let runner = ScriptedRunner::new(&[])
            .deferring(&[program])
            .answering("nothing was attempted: another run holds the lock\n");
        let lane = command_lane(&[program]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert_eq!(
            report.failures, 0,
            "a deferral is not a failure: {report:?}"
        );
        assert!(report.deferred, "{report:?}");
        assert_eq!(
            report.last_failure, None,
            "a deferral is never the alertable failure: {report:?}"
        );
        assert!(
            report.lines.iter().any(|line| line.contains("deferred")),
            "{:?}",
            report.lines
        );
        // Stdout survives a deferral exactly as it survives a failure: a
        // deferring lane explains itself on the way out.
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("another run holds the lock")),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_deferred_lane_carries_its_stderr_explanation_in_the_deferred_line() {
        // D7: the Homebrew job explains contention on STDERR, not stdout, so
        // the deferred line has to carry what run_with_input put in the
        // Verdict, not just whatever the child happened to print on stdout.
        let program = "/usr/local/bin/updater";
        let runner = ScriptedRunner::new(&[]).deferring(&[program]);
        let lane = command_lane(&[program]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert!(
            report.lines.iter().any(|line| line.contains("exit 75")),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_deferred_lines_reason_is_the_childs_own_text_not_a_fixed_string() {
        // A mutant that hardcodes the deferred line's reason regardless of
        // what `Verdict::Deferred` actually carries would still satisfy
        // every OTHER deferred-lane test here: they all defer through
        // `deferring`, whose stub reason is itself the fixed "exit 75". A
        // reason unique to THIS call is the only way to tell the two apart.
        let program = "/usr/local/bin/updater";
        let runner =
            ScriptedRunner::new(&[]).deferring_because(&[program], "UNIQUE-DEFER-REASON-42");
        let lane = command_lane(&[program]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("UNIQUE-DEFER-REASON-42")),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_child_that_could_not_be_run_is_a_failure_naming_the_program() {
        let program = "/no/such/uu-command-lane-test-program";
        let runner = ScriptedRunner::new(&[]).unable_to_run(&[program]);
        let lane = command_lane(&[program]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .last_failure
                .as_ref()
                .is_some_and(|failure| failure.contains(program)),
            "{:?}",
            report.last_failure
        );
    }

    #[test]
    fn a_talkative_child_keeps_its_last_lines_and_says_how_many_were_dropped() {
        let lines: Vec<String> = (1..=25).map(|number| format!("line {number}")).collect();
        let runner = ScriptedRunner::new(&[]).answering(&format!("{}\n", lines.join("\n")));
        let lane = command_lane(&["/bin/x"]);
        let report = lane.run("mine", &stub_facts(), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            report.lines.len(),
            STDOUT_LINES_KEPT + 1,
            "{:?}",
            report.lines
        );
        assert_eq!(report.lines[0], "... 5 earlier line(s) dropped");
        assert_eq!(report.lines.last(), Some(&"line 25".to_string()));
        assert!(
            !report.lines.contains(&"line 1".to_string()),
            "{:?}",
            report.lines
        );
    }
}
