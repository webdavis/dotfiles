//! What one step of the lane contributes to the report.
//!
//! CONTINUE ON FAILURE, like every lane: a step that failed is counted, named
//! and left behind, because the next attempt is a week away and a run that
//! aborts at its first problem throws away every subject it had not reached.

use std::time::Duration;

use crate::lanes::{CommandRunner, LaneReport};

/// What the record says about one step. THE REASON, not only the status:
/// `exit 1` alone sends the operator to a log the week may have rotated away,
/// and the command already said why on stderr.
///
/// A step's own STDOUT is not kept. `brew upgrade` prints a build log, and one
/// lane's output must not crowd every other lane out of the record; the change
/// sections are what say what the week did.
pub fn note(report: &mut LaneReport, label: &str, outcome: Result<String, String>) {
    match outcome {
        Ok(_) => report.noted(format!("{label}: ok")),
        Err(why) => report.failed(format!("{label}: {why}")),
    }
}

/// One command, bounded only by what is left of the lane's own deadline.
pub fn step(
    report: &mut LaneReport,
    runner: &dyn CommandRunner,
    label: &str,
    program: &str,
    args: &[&str],
) {
    note(report, label, runner.run(program, args));
}

/// One command under a bound of its own, for a subject that WEDGES rather
/// than fails. The App Store hangs indefinitely on a broken session, and the
/// lane deadline covers the whole lane, so an unbounded mas step is a week in
/// which nothing after it ran rather than one failed step.
pub fn bounded_step(
    report: &mut LaneReport,
    runner: &dyn CommandRunner,
    label: &str,
    program: &str,
    args: &[&str],
    most: Duration,
) {
    note(report, label, runner.run_with_deadline(program, args, most));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::tests::ScriptedRunner;

    fn report() -> LaneReport {
        LaneReport::new("brew")
    }

    #[test]
    fn a_step_that_failed_carries_the_reason_the_command_gave_not_only_its_label() {
        let mut report = report();
        note(
            &mut report,
            "brew upgrade",
            Err("exit 1: no such tap".into()),
        );
        assert_eq!(report.failures, 1);
        assert_eq!(report.lines, vec!["brew upgrade: exit 1: no such tap"]);
    }

    #[test]
    fn a_step_that_worked_is_recorded_without_the_build_log_it_printed() {
        let mut report = report();
        note(&mut report, "brew upgrade", Ok("pages of output".into()));
        assert_eq!(report.failures, 0);
        assert_eq!(report.lines, vec!["brew upgrade: ok"]);
    }

    #[test]
    fn an_ordinary_step_runs_the_program_and_its_arguments_under_no_bound_of_its_own() {
        let runner = ScriptedRunner::new(&[]);
        let mut report = report();
        step(&mut report, &runner, "brew update", "/b/brew", &["update"]);
        assert_eq!(runner.calls(), vec![vec!["/b/brew", "update"]]);
        assert!(runner.deadlines().is_empty(), "{:?}", runner.deadlines());
    }

    #[test]
    fn a_bounded_step_is_given_its_own_deadline_rather_than_the_lanes() {
        let runner = ScriptedRunner::new(&[]);
        let mut report = report();
        bounded_step(
            &mut report,
            &runner,
            "mas upgrade",
            "/b/mas",
            &["upgrade"],
            Duration::from_secs(180),
        );
        assert_eq!(
            runner.deadlines(),
            vec![(
                vec!["/b/mas".to_string(), "upgrade".to_string()],
                Duration::from_secs(180)
            )]
        );
    }
}
