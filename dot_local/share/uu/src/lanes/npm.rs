//! The npm lane: every globally installed npm package upgraded, weekly, on
//! FNM'S NODE.
//!
//! THE PATH IS THE WHOLE PROBLEM. npm is an `#!/usr/bin/env node` script, so
//! whichever node PATH answers with is the node it runs on and the prefix it
//! installs into. Point the lane at fnm's npm and run it with some other
//! node's directory first and the upgrade lands in that other node's prefix,
//! silently. So the child runs with the directory npm itself sits in AHEAD of
//! everything uu inherited, which is the same dir fnm's node sits in.
//!
//! WHY A SHELL, when every other lane execs its program directly: the child's
//! PATH has to be this directory PLUS whatever uu inherited (the plist's own
//! PATH under the weekly job), and neither half is available here. The spawn
//! seam passes argv and nothing else, and this crate's decision modules read
//! no environment at all. `sh` composes the two at exec time, is handed the
//! directory as a positional argument rather than spliced into its script, and
//! `exec`s npm in place, so nothing extra is left in the process group the
//! deadline kills.
//!
//! AN ABSENT npm IS A FAILURE HERE, deliberately unlike the bash weekly job
//! this ports from, which printed "nothing to upgrade" and returned clean when
//! the binary was missing. A machine that declares the lane and has no npm at
//! the path it declared is a machine whose global packages stopped being
//! upgraded, and the record is where that has to show up.

use crate::config::NpmLane;
use crate::lanes::{CommandRunner, LaneReport};

/// The shell that composes the child's PATH, at its POSIX path.
const SHELL: &str = "/bin/sh";

/// The child's whole program: put `$1` first on PATH, then run the command in
/// the rest of the arguments in place. `$1` is passed in, never spliced into
/// this text, so a directory with a space or a quote in it stays one word.
///
/// `${PATH:+:$PATH}` AND NOT `:$PATH`, because a PATH that is set and empty
/// would otherwise compose `<dir>:`, and an EMPTY PATH ELEMENT IS THE WORKING
/// DIRECTORY: every helper the child shells out to and does not find in
/// `<dir>` would then be answered from wherever uu was started. What this
/// cannot reach is a PATH that is not in the environment at all, where the
/// shell substitutes its own default before this line runs (bash's ends in
/// `.`, the same hazard); the launchd job that carries this lane states a
/// PATH, so that case is the shell's to answer and not this script's.
const PREPEND_PATH: &str = r#"PATH="$1${PATH:+:$PATH}"; export PATH; shift; exec "$@""#;

/// Upgrade every global npm package, and report what that took.
pub fn run_npm(name: &str, lane: &NpmLane, runner: &dyn CommandRunner) -> LaneReport {
    let mut report = LaneReport::new(name);
    let binary = lane.binary.as_str();
    match runner.run(
        SHELL,
        &[
            "-c",
            PREPEND_PATH,
            "sh",
            bin_dir(binary),
            binary,
            "update",
            "-g",
        ],
    ) {
        // npm narrates its upgrades on stdout, but a week with nothing to
        // upgrade prints nothing at all, so this line is what says the lane
        // ran.
        Ok(_) => report.noted(format!("{binary} update -g: ok")),
        Err(why) => report.failed(format!("{binary} update -g FAILED ({why})")),
    }
    report
}

/// The directory `binary` sits in, which is what goes first on the child's
/// PATH. The config refuses anything but an absolute path, so there is always
/// a directory to name; a binary directly under the root names the root
/// itself rather than the empty string, which PATH reads as the working
/// directory.
fn bin_dir(binary: &str) -> &str {
    match binary.rsplit_once('/') {
        Some(("", _)) => "/",
        Some((dir, _)) => dir,
        None => ".",
    }
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
            unreachable!("the npm lane hands its child nothing on stdin")
        }
    }

    /// The shipped configuration's own npm: fnm's version-free bin dir.
    const NPM: &str = "/Users/someone/.local/share/fnm/aliases/default/bin/npm";

    fn lane() -> NpmLane {
        NpmLane {
            binary: NPM.to_string(),
        }
    }

    #[test]
    fn the_lane_upgrades_every_global_package_with_the_npm_it_was_pointed_at() {
        let runner = StubRunner::clean();
        run_npm("npm", &lane(), &runner);
        let call = runner.calls().first().expect("the lane runs npm").clone();
        assert_eq!(call[0], SHELL);
        assert_eq!(&call[call.len() - 3..], [NPM, "update", "-g"]);
    }

    #[test]
    fn the_child_runs_with_npms_own_directory_ahead_of_the_inherited_path() {
        // The bug this exists for: npm on another node's PATH installs into
        // that node's prefix. The directory goes in as an argument, so the
        // whole composition is `$1` first, then whatever uu inherited.
        let runner = StubRunner::clean();
        run_npm("npm", &lane(), &runner);
        assert_eq!(
            runner.calls(),
            vec![
                [
                    SHELL,
                    "-c",
                    r#"PATH="$1${PATH:+:$PATH}"; export PATH; shift; exec "$@""#,
                    "sh",
                    "/Users/someone/.local/share/fnm/aliases/default/bin",
                    NPM,
                    "update",
                    "-g",
                ]
                .map(String::from)
                .to_vec()
            ]
        );
    }

    #[test]
    fn the_directory_put_first_is_the_one_the_npm_binary_sits_in() {
        assert_eq!(bin_dir("/opt/node/bin/npm"), "/opt/node/bin");
        // A binary directly under the root names the root, never the empty
        // string: an empty PATH entry is the WORKING DIRECTORY, so that
        // spelling would hand the child whatever it happened to start in.
        assert_eq!(bin_dir("/npm"), "/");
    }

    #[test]
    fn an_inherited_path_that_is_empty_leaves_the_child_no_empty_entry() {
        // THE ONE TEST HERE THAT RUNS A REAL SHELL, because the property
        // belongs to the script's text and not to the argv around it. An
        // empty PATH element is the working directory, so the `:$PATH`
        // spelling would hand a child on a machine with an empty PATH
        // whatever uu happened to be started in.
        let composed = std::process::Command::new(SHELL)
            .args([
                "-c",
                PREPEND_PATH,
                "sh",
                "/fnm/bin",
                "/bin/sh",
                "-c",
                r#"printf %s "$PATH""#,
            ])
            .env("PATH", "")
            .output()
            .expect("/bin/sh runs");
        assert_eq!(String::from_utf8_lossy(&composed.stdout), "/fnm/bin");
    }

    #[test]
    fn a_clean_upgrade_is_one_recorded_line_and_no_failure() {
        let report = run_npm("npm", &lane(), &StubRunner::clean());
        assert_eq!(report.failures, 0);
        assert_eq!(report.last_failure, None);
        assert_eq!(report.lines, vec![format!("{NPM} update -g: ok")]);
    }

    #[test]
    fn an_upgrade_that_did_not_succeed_is_a_counted_failure_carrying_what_npm_said() {
        let report = run_npm(
            "npm",
            &lane(),
            &StubRunner::refusing("exit 1: npm error code EACCES"),
        );
        assert_eq!(report.failures, 1);
        let line = report.last_failure.expect("a failure names itself");
        assert!(line.contains("exit 1: npm error code EACCES"), "{line}");
        assert_eq!(report.lines, vec![line]);
    }

    #[test]
    fn an_npm_that_is_not_installed_is_a_failure_rather_than_a_quiet_skip() {
        // The bash job's own behavior, deliberately not ported: it printed
        // "npm is not at ...; nothing to upgrade" and returned 0.
        let report = run_npm(
            "npm",
            &lane(),
            &StubRunner::refusing("exit 127: sh: npm: No such file or directory"),
        );
        assert_eq!(report.failures, 1);
        assert!(
            report
                .last_failure
                .is_some_and(|line| line.contains("No such file or directory")),
            "an absent npm must name itself in the record"
        );
    }
}
