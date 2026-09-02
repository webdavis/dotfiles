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

use crate::config::{CommandLane, Config, HerdrLane, LaneKind};
use crate::record::{RunFacts, lane_event};

/// What one lane did: how many things went wrong, whether it DEFERRED instead
/// of running, the lines the record carries about it, and the last of those
/// lines that reported a FAILURE.
///
/// DEFERRED IS NOT A FAILURE. A lane that exited `DEFERRED_EXIT_CODE` did not
/// run at all; that is a fact worth a distinct line in the record, and never
/// a reason to alert or to count toward `failures`.
///
/// THE LAST FAILURE IS KEPT SEPARATELY because the lane continues past one,
/// so the last line written is routinely a later success. The alert has room
/// for one sentence and it has to be the one naming what to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneReport {
    pub name: String,
    pub failures: usize,
    pub deferred: bool,
    pub lines: Vec<String>,
    pub last_failure: Option<String>,
}

impl LaneReport {
    /// A report for a lane that has not done anything yet.
    pub fn new(name: &str) -> Self {
        LaneReport {
            name: name.to_string(),
            failures: 0,
            deferred: false,
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

    /// The lane DEFERRED: nothing was attempted, so this is recorded rather
    /// than counted as a failure and never fires the per-run alert. Distinct
    /// from `failed`, which the caller must never also call for the same
    /// verdict: a lane either deferred or it did not.
    pub fn deferred(&mut self, line: String) {
        self.deferred = true;
        self.lines.push(line);
    }

    /// One thing that went right, or a fact the record carries.
    pub fn noted(&mut self, line: String) {
        self.lines.push(line);
    }
}

/// The exit code the two weekly jobs this ported from already use to mean
/// "nothing was attempted, try later" (a serialize-lock EX_TEMPFAIL). Matching
/// it is the whole point of this verdict: a lane exiting anything else stays a
/// failure.
///
/// THE COLLISION IS REAL BUT NARROW. The system header defines 75 only as a
/// generic temporary failure, and hermes itself already uses 75 for a
/// completed graceful gateway response, so a lane whose PROGRAM IS hermes, or
/// which propagates hermes's own exit code unchanged, could exit 75 for a
/// reason that has nothing to do with deferral. Verified against both
/// existing weekly jobs (2026-09-02): neither propagates an inner hermes exit
/// code outward. Each calls hermes inside a bash `if ...; then ... else
/// ...; fi`, and each job's own exit status comes from its own explicit `exit
/// N` statements alone, never from `$?` after a hermes call. A future
/// `command` lane whose `run` is hermes itself, or that forwards hermes's own
/// status unchanged, would collide; the shipped config template says so.
pub const DEFERRED_EXIT_CODE: i32 = 75;

/// What a command lane's child did, when it could be run at all. `stdout` is
/// kept EVEN ON A NON-CLEAN EXIT (a failed or deferred child's own record
/// lines are not the thing that failed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub stdout: String,
    pub verdict: Verdict,
}

/// How a command lane's child ended. `Deferred` and `Failed` each carry the
/// one line `failure_reason` composes (how it ended, plus the tail of what it
/// said on stderr): a deferring lane explains itself on stderr as often as a
/// failing one does, and that explanation belongs in the record either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Clean,
    Deferred(String),
    Failed(String),
}

/// The spawn seam. `run`'s `Ok` carries the command's stdout, `Err` why it did
/// not succeed, already fit to print.
///
/// `run_with_input` is for a child that is HANDED something on stdin (a
/// command lane's run event): it separates "could not run this at all" (the
/// `Err`, e.g. a missing executable) from "ran, but did not exit clean"
/// (`Ran::verdict`), because the second case still has stdout worth
/// recording.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;
    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String>;
}

/// How much of a failed command's stderr a lane line carries.
pub const STDERR_TAIL: usize = 240;

/// The last `keep` characters of `text`, prefixed with `...` when it was cut.
/// Shared by `failure_reason`'s stderr tail and a command lane's own stdout
/// cap: BOUNDED because both go into the record and into one alert card, and
/// the verdict a tool prints is at the END of what it said.
pub fn tail(text: &str, keep: usize) -> String {
    let length = text.chars().count();
    if length <= keep {
        return text.to_string();
    }
    let cut = text
        .char_indices()
        .nth(length - keep)
        .map_or(text.len(), |(index, _)| index);
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
    format!("{how_it_ended}: {}", tail(&squash(said), STDERR_TAIL))
}

/// How many of a command lane's last stdout lines the record keeps.
///
/// 20 LINES AT `STDERR_TAIL` (240) CHARACTERS EACH IS 4,800 CHARACTERS, chosen
/// against the Discord adapter that chunks a message at 2000 characters: a
/// talkative child at the cap spans about three of those messages rather than
/// dozens, so one command lane's stdout cannot crowd every other lane's line
/// out of the record.
const STDOUT_LINES_KEPT: usize = 20;

/// The last `STDOUT_LINES_KEPT` non-empty lines of a command lane's stdout,
/// each squashed to one line and cut to `STDERR_TAIL` characters, with a
/// count of what was dropped when there was more than that to keep.
///
/// NON-EMPTY, because a talkative child pads its output with blank lines that
/// would otherwise crowd out the ones that say something.
fn stdout_lines(stdout: &str) -> Vec<String> {
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let dropped = lines.len().saturating_sub(STDOUT_LINES_KEPT);
    let mut kept: Vec<String> = lines
        .iter()
        .skip(dropped)
        .map(|line| tail(&squash(line), STDERR_TAIL))
        .collect();
    if dropped > 0 {
        kept.insert(0, format!("... {dropped} earlier line(s) dropped"));
    }
    kept
}

/// Text with every control character (embedded CR, stray control bytes)
/// mapped to a space and every backtick mapped to a plain quote, shared by
/// `failure_reason` and a command lane's stdout lines: untrusted child
/// output crosses into the record and out to Discord, where a control
/// character would reflow or truncate a line and three backticks would open
/// or close a code fence around every record line after it.
fn squash(line: &str) -> String {
    line.chars()
        .map(|letter| match letter {
            _ if letter.is_control() => ' ',
            '`' => '\'',
            _ => letter,
        })
        .collect()
}

/// The command lane: hands the run event to the child on stdin under the
/// locked contract, keeps its stdout as record lines, and turns the child's
/// verdict, or a child that could not run at all, into what the record and
/// the alert summary say.
///
/// STDOUT IS KEPT EVEN ON A NON-CLEAN EXIT. `run_with_input`'s `Ran::verdict`
/// already carries the reason (the exit description and the stderr tail);
/// what the child printed on the way there is still worth recording, and
/// `report.noted` runs before `report.failed`/`report.deferred` so it does.
///
/// THE CHILD'S WORLD: `run[0]` is the program, `run[1..]` its arguments, and
/// argv[0] the child sees is `run[0]` verbatim. Env and working directory are
/// INHERITED from uu's own process; under the tracked LaunchAgent that is the
/// plist's own PATH plus HOME, with the working directory at `/`. The child
/// must not leave anything behind holding its stdout or stderr open (a
/// backgrounded process, a detached daemon), or uu waits for it forever:
/// no lane has a deadline, by design (`SystemRunner` in main.rs says why).
pub fn run_command(
    name: &str,
    lane: &CommandLane,
    facts: &RunFacts,
    runner: &dyn CommandRunner,
) -> LaneReport {
    let mut report = LaneReport::new(name);
    let program = lane.run[0].as_str();
    let args: Vec<&str> = lane.run[1..].iter().map(String::as_str).collect();
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
pub fn run_lane(
    name: &str,
    config: &Config,
    facts: &RunFacts,
    runner: &dyn CommandRunner,
) -> Option<LaneReport> {
    match config.lanes.get(name)? {
        // The herdr lane predates the run event and has no use for it.
        LaneKind::Herdr(lane) => Some(run_herdr(name, lane, runner)),
        LaneKind::Command(lane) => Some(run_command(name, lane, facts, runner)),
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
    use crate::record::Marker;
    use std::cell::RefCell;

    /// A runner that answers from a script and records every call. The script
    /// is keyed on the whole argument vector, so a test says exactly which
    /// invocation fails without depending on call order.
    struct ScriptedRunner {
        failing: Vec<Vec<String>>,
        deferring: Vec<Vec<String>>,
        unrunnable: Vec<Vec<String>>,
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
                deferring: Vec::new(),
                unrunnable: Vec::new(),
                stdout: String::new(),
                calls: RefCell::new(Vec::new()),
                inputs: RefCell::new(Vec::new()),
            }
        }

        fn answering(mut self, stdout: &str) -> Self {
            self.stdout = stdout.to_string();
            self
        }

        /// A `run_with_input` call that exits `DEFERRED_EXIT_CODE`, distinct
        /// from `failing`'s "ran, but exited some other non-zero code".
        fn deferring(mut self, call: &[&str]) -> Self {
            self.deferring
                .push(call.iter().map(|word| word.to_string()).collect());
            self
        }

        /// A `run_with_input` call the runner cannot make at all, the
        /// could-not-run path (a missing executable), distinct from
        /// `failing`'s "ran, but exited non-zero".
        fn unable_to_run(mut self, call: &[&str]) -> Self {
            self.unrunnable
                .push(call.iter().map(|word| word.to_string()).collect());
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
            if self.unrunnable.contains(&call) {
                return Err(format!("could not run {program}: stubbed as unrunnable"));
            }
            let verdict = if self.deferring.contains(&call) {
                Verdict::Deferred("exit 75".to_string())
            } else if self.failing.contains(&call) {
                Verdict::Failed("exit 1".to_string())
            } else {
                Verdict::Clean
            };
            Ok(Ran {
                stdout: self.stdout.clone(),
                verdict,
            })
        }
    }

    /// The one fixed `RunFacts` every test here that does not care about its
    /// contents can share; `record.rs` owns the tests that pin `lane_event`
    /// itself against varied facts.
    const STUB_MARKER: Marker = Marker::NeverRecorded;

    fn stub_facts() -> RunFacts<'static> {
        RunFacts {
            host: "test-host",
            started_epoch: 0,
            started_iso: "1970-01-01T00:00:00Z",
            marker: &STUB_MARKER,
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

    fn command_lane(run: &[&str]) -> CommandLane {
        CommandLane {
            run: run.iter().map(|word| word.to_string()).collect(),
        }
    }

    // --- the registry ---------------------------------------------------------

    #[test]
    fn a_config_with_no_lane_block_enables_nothing() {
        let config = parse_config("").unwrap();
        assert!(enabled_lanes(&config).is_empty());
        assert_eq!(
            run_lane("herdr", &config, &stub_facts(), &ScriptedRunner::new(&[])),
            None,
            "a lane with no block must not run just because it was named"
        );
    }

    #[test]
    fn a_lane_block_with_nothing_in_it_turns_the_lane_on() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(enabled_lanes(&config), vec!["herdr"]);
        assert!(run_lane("herdr", &config, &stub_facts(), &ScriptedRunner::new(&[])).is_some());
    }

    #[test]
    fn a_lane_this_build_does_not_have_runs_nothing() {
        let config = parse_config("[lanes.herdr]\n").unwrap();
        assert_eq!(
            run_lane("brew", &config, &stub_facts(), &ScriptedRunner::new(&[])),
            None
        );
    }

    #[test]
    fn every_built_in_lane_type_can_be_selected_and_run_and_keeps_its_own_name() {
        // One minimal block per BUILT-IN TYPE (the WEEKDAY_NAMES pattern): a
        // type in `LANE_TYPES` that dispatches to nothing, or that loses the
        // lane's own name along the way, would accept a lane it never truly
        // runs. `command` needs a `run` to be valid at all, so the block is
        // spelled out per fixture rather than derived from the name alone.
        let fixtures: &[(&str, &str, &str)] = &[
            ("command", "[lanes.command]\nrun = [\"x\"]\n", "command"),
            ("herdr", "[lanes.herdr]\n", "herdr"),
        ];
        assert_eq!(crate::config::LANE_TYPES.len(), fixtures.len());
        for (kind, block, name) in fixtures {
            let config = parse_config(block).unwrap();
            let report = run_lane(name, &config, &stub_facts(), &ScriptedRunner::new(&[]))
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
        let report = run_lane("mine", &config, &stub_facts(), &ScriptedRunner::new(&[])).unwrap();
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
        assert_eq!(
            runner.calls(),
            vec![vec!["cmd", "a", "b"], vec!["cmd", "a", "b"]],
            "the program and its arguments are recorded beside the input"
        );
    }

    #[test]
    fn a_scripted_runner_answers_run_with_input_failure_from_its_failing_set() {
        let runner = ScriptedRunner::new(&[&["cmd", "a"]]);
        let ran = runner.run_with_input("cmd", &["a"], "in\n").unwrap();
        assert_eq!(ran.verdict, Verdict::Failed("exit 1".to_string()));
    }

    #[test]
    fn a_scripted_runner_answers_run_with_input_deferral_from_its_deferring_set() {
        let runner = ScriptedRunner::new(&[]).deferring(&["cmd", "a"]);
        let ran = runner.run_with_input("cmd", &["a"], "in\n").unwrap();
        assert_eq!(ran.verdict, Verdict::Deferred("exit 75".to_string()));
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

    #[test]
    fn tail_cuts_on_a_character_boundary_not_a_byte_offset() {
        // Each "party popper" is 4 bytes. A byte-offset cut (`text.len() -
        // keep`) lands inside the third one's encoding and panics; only a
        // char-aware cut keeps the last 4 CHARACTERS, "\u{1F389}ABC".
        assert_eq!(tail("\u{1F389}\u{1F389}\u{1F389}ABC", 4), "...\u{1F389}ABC");
    }

    #[test]
    fn tail_with_nothing_to_keep_is_only_the_cut_mark() {
        // Asking for zero characters must not hand back the whole text.
        assert_eq!(tail("abc", 0), "...");
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
        // Exact, not a loose upper bound: a mutant that kept only half of
        // STDERR_TAIL would still satisfy "<= STDERR_TAIL + 32" and quietly
        // drop half the promised diagnostic.
        assert_eq!(
            reason.chars().count(),
            "exit 1: ...".chars().count() + STDERR_TAIL,
            "{} characters",
            reason.chars().count()
        );
        assert!(reason.contains("..."), "the cut is visible: {reason}");
    }

    #[test]
    fn a_talkative_command_with_multibyte_stderr_is_cut_on_a_character_boundary() {
        // The same cut, but through stderr that is not one byte per
        // character: a byte-slicing mutant would panic or split a code point
        // instead of keeping whole characters, same failure mode as `tail`
        // itself.
        let noise = format!(
            "{}\u{65e5}\u{672c}\u{8a9e}\u{306e}REASON",
            "\u{3042}".repeat(4000)
        );
        let reason = failure_reason("exit 1", &noise);
        assert!(
            reason.ends_with("\u{65e5}\u{672c}\u{8a9e}\u{306e}REASON"),
            "{reason}"
        );
        assert_eq!(
            reason.chars().count(),
            "exit 1: ...".chars().count() + STDERR_TAIL,
            "{} characters",
            reason.chars().count()
        );
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
                        verdict: Verdict::Clean,
                    }),
                    Err(failure) => Ok(Ran {
                        stdout: String::new(),
                        verdict: Verdict::Failed(failure),
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

    // --- the command lane -------------------------------------------------------

    #[test]
    fn a_command_lane_hands_the_run_event_to_the_child_on_stdin_and_records_what_it_printed() {
        let runner = ScriptedRunner::new(&[]).answering("3 upgraded\n");
        // Two arguments, so a mutant that reverses run[1..] or drops the
        // second one changes what the runner recorded.
        let lane = command_lane(&["/usr/local/bin/updater", "--yes", "--now"]);
        let report = run_command("mine", &lane, &stub_facts(), &runner);
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
        let report = run_command("mine", &lane, &stub_facts(), &runner);
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
        let report = run_command("mine", &lane, &stub_facts(), &runner);
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
        let report = run_command("mine", &lane, &stub_facts(), &runner);
        assert!(
            report.lines.iter().any(|line| line.contains("exit 75")),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_child_that_could_not_be_run_is_a_failure_naming_the_program() {
        let program = "/no/such/uu-command-lane-test-program";
        let runner = ScriptedRunner::new(&[]).unable_to_run(&[program]);
        let lane = command_lane(&[program]);
        let report = run_command("mine", &lane, &stub_facts(), &runner);
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
        let report = run_command("mine", &lane, &stub_facts(), &runner);
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

    // --- stdout_lines -----------------------------------------------------------

    #[test]
    fn stdout_lines_keeps_everything_when_there_is_little_to_drop() {
        assert_eq!(
            stdout_lines("a\nb\n"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn stdout_lines_drops_blank_lines_rather_than_counting_them_as_content() {
        assert_eq!(
            stdout_lines("a\n\n\nb\n"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn stdout_lines_squashes_a_control_character_embedded_in_one_line() {
        assert_eq!(stdout_lines("a\rb\n"), vec!["a b".to_string()]);
    }

    #[test]
    fn stdout_lines_cuts_an_overlong_multibyte_line_to_its_exact_tail() {
        // Every OTHER stdout_lines test here is short enough that dropping the
        // `tail(..., STDERR_TAIL)` cut still passes; a mutant like that needs
        // a line over the 240-character cap, and multibyte so a byte-indexed
        // cut would panic or land mid-character instead of matching this.
        let filler = "é".repeat(300);
        let line = format!("{filler}TAIL-MARKER");
        let expected = format!("...{}TAIL-MARKER", "é".repeat(229));
        assert_eq!(stdout_lines(&format!("{line}\n")), vec![expected]);
    }

    #[test]
    fn squash_replaces_backticks_so_no_child_line_can_open_or_close_a_code_fence() {
        let squashed = squash("before ``` after");
        assert!(!squashed.contains('`'), "{squashed:?}");
    }

    #[test]
    fn stdout_lines_says_so_when_exactly_one_line_was_dropped() {
        // The boundary: one over the cap drops one line, and that one is
        // still announced.
        let text: String = (1..=STDOUT_LINES_KEPT + 1)
            .map(|number| format!("line {number}\n"))
            .collect();
        let kept = stdout_lines(&text);
        assert_eq!(kept.len(), STDOUT_LINES_KEPT + 1, "{kept:?}");
        assert_eq!(kept[0], "... 1 earlier line(s) dropped");
        assert_eq!(kept[1], "line 2");
    }
}
