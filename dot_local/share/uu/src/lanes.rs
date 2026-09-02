//! Running the lanes a config declared, and the first lane's own adapter. The
//! registry itself (`Lanes`, `LaneKind`, and their parsing) lives in `config`;
//! this module only dispatches on a kind and does the work.
//!
//! CONTINUE ON FAILURE, at both levels. A plugin that will not reinstall does
//! not stop the next plugin, and a lane that failed does not stop the next
//! lane: the run completes, the record says what failed, and the exit status
//! stays 0. The scheduler's retry is a whole week away, so a run that aborts
//! at its first problem throws away every subject it had not reached yet.
//!
//! herdr HAS NO `plugin update`, so a refresh is an uninstall followed by a
//! fresh install, which re-pins at the source's tip. A failed install RETRIES
//! ONCE and a plugin that still failed is named loudly, so the record for the
//! week says exactly what is missing. The running server keeps its loaded
//! plugins until restart, so a failure costs the next restart rather than the
//! current session.

use crate::config::{Config, HerdrLane, LaneKind};

/// What one lane did: how many things went wrong, the lines the record
/// carries about it, and the last of those lines that reported a FAILURE.
///
/// THE LAST FAILURE IS KEPT SEPARATELY because the lane continues past one,
/// so the last line written is routinely a later success. The alert has room
/// for one sentence and it has to be the one naming what to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneReport {
    pub name: String,
    pub failures: usize,
    pub lines: Vec<String>,
    pub last_failure: Option<String>,
}

impl LaneReport {
    /// A report for a lane that has not done anything yet.
    pub fn new(name: &str) -> Self {
        LaneReport {
            name: name.to_string(),
            failures: 0,
            lines: Vec::new(),
            last_failure: None,
        }
    }

    /// One thing that went WRONG: counted, recorded and remembered, in one
    /// place. A lane cannot count a failure it did not also make alertable,
    /// which is the drift a second `failures += 1` beside a bare push invites.
    pub fn failed(&mut self, line: String) {
        self.failures += 1;
        self.last_failure = Some(line.clone());
        self.lines.push(line);
    }

    /// One thing that went right, or a fact the record carries.
    pub fn noted(&mut self, line: String) {
        self.lines.push(line);
    }
}

/// What a command lane's child did, when it could be run at all. `stdout` is
/// kept EVEN ON FAILURE (a failed child's own record lines are not the thing
/// that failed); `failure` is the same one-line reason `failure_reason`
/// composes, or `None` for a clean exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub stdout: String,
    pub failure: Option<String>,
}

/// The spawn seam. `run`'s `Ok` carries the command's stdout, `Err` why it did
/// not succeed, already fit to print.
///
/// `run_with_input` is for a child that is HANDED something on stdin (a
/// command lane's run event): it separates "could not run this at all" (the
/// `Err`, e.g. a missing executable) from "ran, but failed" (`Ran::failure`),
/// because the second case still has stdout worth recording.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;
    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String>;
}

/// How much of a failed command's stderr a lane line carries.
pub const STDERR_TAIL: usize = 240;

/// The last `keep` characters of `text`, prefixed with `...` when it was cut.
/// Shared by `failure_reason`'s stderr tail today and a command lane's own
/// stdout cap tomorrow: BOUNDED because both go into the record and into one
/// alert card, and the verdict a tool prints is at the END of what it said.
pub fn tail(text: &str, keep: usize) -> String {
    let length = text.chars().count();
    if length <= keep {
        return text.to_string();
    }
    let cut = text
        .char_indices()
        .nth(length - keep)
        .map_or(0, |(index, _)| index);
    format!("...{}", &text[cut..])
}

/// Why a command failed, in one line: how it ended, and the tail of what it
/// said about it.
///
/// THE STATUS ALONE IS NOT A REASON. `exit 1` sends the operator to a log that
/// a weekly job may have rotated away, while the command already printed the
/// answer on stderr and this is the last moment it exists. Squashed to a
/// single line so it is not reflowed by a build log.
pub fn failure_reason(how_it_ended: &str, stderr: &str) -> String {
    let said = stderr.trim();
    if said.is_empty() {
        return how_it_ended.to_string();
    }
    let one_line: String = said
        .chars()
        .map(|letter| if letter.is_control() { ' ' } else { letter })
        .collect();
    format!("{how_it_ended}: {}", tail(&one_line, STDERR_TAIL))
}

/// The lanes this config declares, in NAME order.
///
/// ORDER IS THE NAME'S, never the file's. `Lanes` is a `BTreeMap`, so this is
/// always sorted regardless of which block the operator happened to type
/// first, and a run whose sequence changes when a block moves is a run nobody
/// can reason about.
pub fn enabled_lanes(config: &Config) -> Vec<&str> {
    config.lanes.keys().map(String::as_str).collect()
}

/// Run one named lane, or `None` when this config declares none by that name.
/// Dispatches on the lane's own kind; a name the parser accepted always
/// carries a kind this build knows how to run, because an unrecognized `type`
/// was already refused at load.
pub fn run_lane(name: &str, config: &Config, runner: &dyn CommandRunner) -> Option<LaneReport> {
    match config.lanes.get(name)? {
        LaneKind::Herdr(lane) => Some(run_herdr(name, lane, runner)),
    }
}

/// The herdr lane: the binary refreshes itself, then every plugin in the
/// roster is reinstalled at its source's tip.
pub fn run_herdr(name: &str, lane: &HerdrLane, runner: &dyn CommandRunner) -> LaneReport {
    let mut report = LaneReport::new(name);

    match runner.run(&lane.binary, &["update"]) {
        Ok(_) => {
            // The version is a COURTESY in the record and never a verdict: a
            // build that will not print its own version still updated.
            let version = runner
                .run(&lane.binary, &["--version"])
                .ok()
                .and_then(|out| out.lines().next().map(str::to_string))
                .filter(|line| !line.is_empty());
            report.noted(match version {
                Some(version) => format!("herdr self-update: ok ({version})"),
                None => "herdr self-update: ok".to_string(),
            });
        }
        Err(why) => report.failed(format!(
            "herdr self-update FAILED ({why}); plugins still refresh below"
        )),
    }

    for plugin in &lane.plugins {
        let id = plugin.id.as_str();
        // AN INSTALL OVER A FAILED UNINSTALL IS NOT ATTEMPTED. herdr pins a
        // plugin at install, so installing on top of a copy that would not
        // come off is how one plugin becomes two.
        if let Err(why) = runner.run(&lane.binary, &["plugin", "uninstall", id]) {
            report.failed(format!(
                "plugin {id}: uninstall failed ({why}); leaving the installed copy alone"
            ));
            continue;
        }
        let install = || runner.run(&lane.binary, &["plugin", "install", &plugin.repo, "--yes"]);
        // The retry is the SECOND call and there is no third: `or_else` runs
        // it only on a failure, and the reason kept is the one the last
        // attempt gave.
        match install().or_else(|_| install()) {
            Ok(_) => report.noted(format!("plugin {id}: refreshed")),
            Err(why) => report.failed(format!(
                "plugin {id}: REINSTALL FAILED twice ({why}); it is now MISSING until the next \
                 apply or run"
            )),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Plugin, parse_config};
    use std::cell::RefCell;

    /// A runner that answers from a script and records every call. The script
    /// is keyed on the whole argument vector, so a test says exactly which
    /// invocation fails without depending on call order.
    struct ScriptedRunner {
        failing: Vec<Vec<String>>,
        stdout: String,
        calls: RefCell<Vec<Vec<String>>>,
        inputs: RefCell<Vec<String>>,
    }

    impl ScriptedRunner {
        fn new(failing: &[&[&str]]) -> Self {
            ScriptedRunner {
                failing: failing
                    .iter()
                    .map(|call| call.iter().map(|word| word.to_string()).collect())
                    .collect(),
                stdout: String::new(),
                calls: RefCell::new(Vec::new()),
                inputs: RefCell::new(Vec::new()),
            }
        }

        fn answering(mut self, stdout: &str) -> Self {
            self.stdout = stdout.to_string();
            self
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        /// Every `input` a call to `run_with_input` was given, in call order:
        /// the spy `run_with_input` itself never touches (BRIEF U8).
        fn inputs(&self) -> Vec<String> {
            self.inputs.borrow().clone()
        }
    }

    impl CommandRunner for ScriptedRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            let mut call = vec![program.to_string()];
            call.extend(args.iter().map(|word| word.to_string()));
            self.calls.borrow_mut().push(call.clone());
            // The Nth repeat of a scripted failure is still a failure: the
            // retry has to be able to fail too.
            if self.failing.contains(&call) {
                return Err("exit 1".to_string());
            }
            Ok(self.stdout.clone())
        }

        fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String> {
            let mut call = vec![program.to_string()];
            call.extend(args.iter().map(|word| word.to_string()));
            self.calls.borrow_mut().push(call.clone());
            self.inputs.borrow_mut().push(input.to_string());
            Ok(Ran {
                stdout: self.stdout.clone(),
                failure: self.failing.contains(&call).then(|| "exit 1".to_string()),
            })
        }
    }

    fn lane(plugins: &[(&str, &str)]) -> HerdrLane {
        HerdrLane {
            binary: "herdr".to_string(),
            plugins: plugins
                .iter()
                .map(|(id, repo)| Plugin {
                    id: id.to_string(),
                    repo: repo.to_string(),
                })
                .collect(),
        }
    }

    // --- the registry ---------------------------------------------------------

    #[test]
    fn a_config_with_no_lane_block_enables_nothing() {
        let config = parse_config("").unwrap();
        assert!(enabled_lanes(&config).is_empty());
        assert_eq!(
            run_lane("herdr", &config, &ScriptedRunner::new(&[])),
            None,
            "a lane with no block must not run just because it was named"
        );
    }

    #[test]
    fn a_lane_block_with_nothing_in_it_turns_the_lane_on() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(enabled_lanes(&config), vec!["herdr"]);
        assert!(run_lane("herdr", &config, &ScriptedRunner::new(&[])).is_some());
    }

    #[test]
    fn a_lane_this_build_does_not_have_runs_nothing() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(run_lane("brew", &config, &ScriptedRunner::new(&[])), None);
    }

    #[test]
    fn every_built_in_lane_type_can_be_selected_and_run_and_keeps_its_own_name() {
        // One minimal block per BUILT-IN TYPE (the WEEKDAY_NAMES pattern): a
        // type in `LANE_TYPES` that dispatches to nothing, or that loses the
        // lane's own name along the way, would accept a lane it never truly
        // runs.
        let fixtures: &[(&str, &str)] = &[("herdr", "herdr")];
        assert_eq!(crate::config::LANE_TYPES.len(), fixtures.len());
        for (kind, name) in fixtures {
            let config = parse_config(&format!("[lanes.{name}]\n")).unwrap();
            let report = run_lane(name, &config, &ScriptedRunner::new(&[]))
                .unwrap_or_else(|| panic!("the roster names `{kind}` but nothing dispatches it"));
            assert_eq!(&report.name, name);
        }
    }

    #[test]
    fn a_run_lanes_report_carries_the_lanes_own_name_not_its_type() {
        // The fixture above names its herdr lane "herdr", which cannot tell a
        // report carrying the ACTUAL name apart from one hardcoding the type's
        // own literal. A lane named something else closes that gap.
        let config = parse_config("[lanes.mine]\ntype = \"herdr\"\n").unwrap();
        let report = run_lane("mine", &config, &ScriptedRunner::new(&[])).unwrap();
        assert_eq!(report.name, "mine");
    }

    #[test]
    fn lanes_run_in_name_order_whatever_the_file_order() {
        let config =
            parse_config("[lanes.zeta]\ntype = \"herdr\"\n\n[lanes.alpha]\ntype = \"herdr\"\n")
                .unwrap();
        assert_eq!(enabled_lanes(&config), vec!["alpha", "zeta"]);
    }

    // --- the run_with_input spy -------------------------------------------------

    #[test]
    fn the_scripted_runner_records_every_input_it_was_given() {
        // A command lane's own event only ever reaches a real `SystemRunner`
        // through `run_with_input`; this is what a lane test asserts against
        // instead of a real child process (BRIEF U8, kills args-dropped and
        // stdin-not-written at the lane level, above `SystemRunner` itself).
        let runner = ScriptedRunner::new(&[]);
        runner
            .run_with_input("cmd", &["a", "b"], "first\n")
            .unwrap();
        runner.run_with_input("cmd", &["a", "b"], "second\n").ok();
        assert_eq!(runner.inputs(), vec!["first\n", "second\n"]);
    }

    #[test]
    fn a_scripted_runner_answers_run_with_input_failure_from_its_failing_set() {
        let runner = ScriptedRunner::new(&[&["cmd", "a"]]);
        let ran = runner.run_with_input("cmd", &["a"], "in\n").unwrap();
        assert_eq!(ran.failure.as_deref(), Some("exit 1"));
    }

    // --- the shared tail -------------------------------------------------------

    #[test]
    fn tail_returns_the_text_unchanged_when_it_already_fits() {
        assert_eq!(tail("hello", 10), "hello");
    }

    #[test]
    fn tail_keeps_the_last_keep_characters_and_prefixes_the_cut() {
        assert_eq!(tail("0123456789ABCDEF", 4), "...CDEF");
    }

    // --- why a command failed -------------------------------------------------

    #[test]
    fn a_failure_reason_carries_what_the_command_printed_on_stderr() {
        assert_eq!(
            failure_reason("exit 1", "fatal: repository not found\n"),
            "exit 1: fatal: repository not found"
        );
    }

    #[test]
    fn a_command_that_said_nothing_reports_only_how_it_ended() {
        // An empty stderr must not leave a dangling colon with nothing after
        // it, which reads as a message that went missing.
        for silence in ["", "\n", "   \n\t"] {
            assert_eq!(failure_reason("exit 2", silence), "exit 2");
        }
        assert_eq!(
            failure_reason("killed by a signal", ""),
            "killed by a signal"
        );
    }

    #[test]
    fn a_talkative_command_is_cut_to_the_tail_that_holds_its_verdict() {
        // A build log's worth of stderr would push every other line off the
        // record and blow past the one card an alert gets, and the verdict is
        // at the END of it.
        let noise = format!("{}THE REAL REASON", "x".repeat(4000));
        let reason = failure_reason("exit 1", &noise);
        assert!(reason.ends_with("THE REAL REASON"), "{reason}");
        assert!(
            reason.chars().count() <= STDERR_TAIL + 32,
            "{} characters",
            reason.chars().count()
        );
        assert!(reason.contains("..."), "the cut is visible: {reason}");
    }

    #[test]
    fn a_multi_line_stderr_is_squashed_onto_one_line() {
        // Lane lines are indented under their lane in the record and go out as
        // one alert sentence; an embedded newline breaks both.
        let reason = failure_reason("exit 1", "first\nsecond\r\nthird");
        assert!(!reason.contains('\n'), "{reason}");
        assert!(!reason.contains('\r'), "{reason}");
        assert!(reason.contains("third"), "{reason}");
    }

    // --- the herdr lane -------------------------------------------------------

    #[test]
    fn a_clean_run_updates_herdr_then_refreshes_every_plugin_in_roster_order() {
        let runner = ScriptedRunner::new(&[]);
        let report = run_herdr("herdr", &lane(&[("a", "o/a"), ("b", "o/b")]), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            runner.calls(),
            vec![
                vec!["herdr", "update"],
                vec!["herdr", "--version"],
                vec!["herdr", "plugin", "uninstall", "a"],
                vec!["herdr", "plugin", "install", "o/a", "--yes"],
                vec!["herdr", "plugin", "uninstall", "b"],
                vec!["herdr", "plugin", "install", "o/b", "--yes"],
            ]
        );
    }

    #[test]
    fn the_configured_binary_is_the_one_that_runs() {
        let runner = ScriptedRunner::new(&[]);
        let mut configured = lane(&[]);
        configured.binary = "/opt/herdr".to_string();
        run_herdr("herdr", &configured, &runner);
        assert_eq!(runner.calls()[0][0], "/opt/herdr");
    }

    #[test]
    fn a_failed_self_update_is_counted_and_the_plugins_still_refresh() {
        let runner = ScriptedRunner::new(&[&["herdr", "update"]]);
        let report = run_herdr("herdr", &lane(&[("a", "o/a")]), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("self-update FAILED")),
            "{:?}",
            report.lines
        );
        assert!(
            report
                .lines
                .iter()
                .any(|line| line == "plugin a: refreshed"),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_successful_self_update_reports_the_version_it_landed_on() {
        let runner = ScriptedRunner::new(&[]).answering("herdr 0.42.0\nbuilt from source\n");
        let report = run_herdr("herdr", &lane(&[]), &runner);
        assert_eq!(report.lines[0], "herdr self-update: ok (herdr 0.42.0)");
    }

    #[test]
    fn a_version_that_will_not_answer_still_leaves_the_update_reported_as_ok() {
        // The version is a courtesy in the record, never a verdict: counting it
        // as a failure would report a healthy update as a broken one.
        let runner = ScriptedRunner::new(&[&["herdr", "--version"]]);
        let report = run_herdr("herdr", &lane(&[]), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(report.lines[0], "herdr self-update: ok");
    }

    #[test]
    fn a_plugin_whose_uninstall_fails_is_left_installed_and_never_reinstalled() {
        // Installing over a plugin the uninstall could not remove is how a
        // half-removed plugin becomes two.
        let runner = ScriptedRunner::new(&[&["herdr", "plugin", "uninstall", "a"]]);
        let report = run_herdr("herdr", &lane(&[("a", "o/a"), ("b", "o/b")]), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("plugin a: uninstall failed")),
            "{:?}",
            report.lines
        );
        assert!(
            !runner
                .calls()
                .iter()
                .any(|call| call.contains(&"o/a".to_string())),
            "{:?}",
            runner.calls()
        );
        // And the next plugin is still refreshed.
        assert!(
            report
                .lines
                .iter()
                .any(|line| line == "plugin b: refreshed"),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_failed_install_is_retried_exactly_once_and_a_second_attempt_that_works_is_a_refresh() {
        struct FlakyInstall {
            attempts: RefCell<usize>,
        }
        impl CommandRunner for FlakyInstall {
            fn run(&self, _program: &str, args: &[&str]) -> Result<String, String> {
                if args.first() == Some(&"plugin") && args.get(1) == Some(&"install") {
                    let mut attempts = self.attempts.borrow_mut();
                    *attempts += 1;
                    if *attempts == 1 {
                        return Err("exit 1".to_string());
                    }
                }
                Ok(String::new())
            }

            fn run_with_input(
                &self,
                program: &str,
                args: &[&str],
                _input: &str,
            ) -> Result<Ran, String> {
                match self.run(program, args) {
                    Ok(stdout) => Ok(Ran {
                        stdout,
                        failure: None,
                    }),
                    Err(failure) => Ok(Ran {
                        stdout: String::new(),
                        failure: Some(failure),
                    }),
                }
            }
        }
        let runner = FlakyInstall {
            attempts: RefCell::new(0),
        };
        let report = run_herdr("herdr", &lane(&[("a", "o/a")]), &runner);
        assert_eq!(
            *runner.attempts.borrow(),
            2,
            "one retry, not none and not a loop"
        );
        assert_eq!(report.failures, 0);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line == "plugin a: refreshed")
        );
    }

    #[test]
    fn a_plugin_that_fails_twice_is_named_loudly_as_missing() {
        let runner = ScriptedRunner::new(&[&["herdr", "plugin", "install", "o/a", "--yes"]]);
        let report = run_herdr("herdr", &lane(&[("a", "o/a")]), &runner);
        assert_eq!(report.failures, 1);
        assert!(
            report
                .lines
                .iter()
                .any(|line| line.contains("plugin a: REINSTALL FAILED twice")
                    && line.contains("MISSING")),
            "{:?}",
            report.lines
        );
        assert_eq!(
            runner
                .calls()
                .iter()
                .filter(|call| call.contains(&"install".to_string()))
                .count(),
            2
        );
    }

    #[test]
    fn a_failure_followed_by_a_success_is_summarized_by_the_failure() {
        // The lane CONTINUES after a failure, so its last line is routinely a
        // later success. An alert reading `1 failure(s); plugin a: refreshed`
        // is a card that names nothing to fix and reads like a lane that
        // worked.
        let runner = ScriptedRunner::new(&[&["herdr", "update"]]);
        let report = run_herdr("herdr", &lane(&[("a", "o/a")]), &runner);
        let summary = crate::alert::alert_summary(&report);
        assert!(summary.contains("self-update FAILED"), "{summary}");
        assert!(!summary.contains("refreshed"), "{summary}");
    }

    #[test]
    fn a_failed_step_names_the_reason_the_command_gave() {
        // `exit 1` alone sends the operator to a log a weekly job may have
        // rotated away. The command already said why on stderr, and the
        // record and the alert are where that has to end up.
        let runner = ScriptedRunner::new(&[
            &["herdr", "plugin", "uninstall", "a"],
            &["herdr", "plugin", "install", "o/b", "--yes"],
        ]);
        let report = run_herdr("herdr", &lane(&[("a", "o/a"), ("b", "o/b")]), &runner);
        for line in report
            .lines
            .iter()
            .filter(|line| line.contains("FAILED") || line.contains("failed"))
        {
            assert!(line.contains("exit 1"), "{line}");
        }
        assert_eq!(report.failures, 2);
    }

    #[test]
    fn every_failure_across_the_lane_is_counted_once() {
        let runner = ScriptedRunner::new(&[
            &["herdr", "update"],
            &["herdr", "plugin", "uninstall", "a"],
            &["herdr", "plugin", "install", "o/b", "--yes"],
        ]);
        let report = run_herdr(
            "herdr",
            &lane(&[("a", "o/a"), ("b", "o/b"), ("c", "o/c")]),
            &runner,
        );
        assert_eq!(report.failures, 3);
        assert_eq!(report.name, "herdr");
    }

    #[test]
    fn a_lane_with_no_plugins_still_updates_the_binary() {
        let runner = ScriptedRunner::new(&[]);
        let report = run_herdr("herdr", &lane(&[]), &runner);
        assert_eq!(report.failures, 0);
        assert_eq!(
            runner.calls().first().map(|call| call[1].clone()),
            Some("update".to_string())
        );
    }
}
