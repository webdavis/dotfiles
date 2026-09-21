//! The npm lane: every globally installed npm package upgraded, weekly, on
//! FNM'S NODE.
//!
//! THE PATH IS THE WHOLE PROBLEM. npm is an `#!/usr/bin/env node` script, so
//! whichever node PATH answers with is the node it runs on and the prefix it
//! installs into. Point the lane at fnm's npm and run it with some other
//! node's directory first and the upgrade lands in that other node's prefix,
//! silently. So the child runs with the directory npm itself sits in AHEAD of
//! everything uu inherited, which is the same dir fnm's node sits in. The
//! lane names that directory and the spawn seam joins it to the inherited
//! value, which uu's own process is the only place to read.
//!
//! AN ABSENT npm IS A FAILURE HERE, deliberately unlike the bash weekly job
//! this ports from, which printed "nothing to upgrade" and returned clean when
//! the binary was missing. A machine that declares the lane and has no npm at
//! the path it declared is a machine whose global packages stopped being
//! upgraded, and the record is where that has to show up.

use crate::config::NpmLane;
use crate::lanes::{CommandRunner, Environment, LaneAdapter};
use uu_domain::LaneReport;
use uu_domain::RunFacts;

/// Upgrade every global npm package, and report what that took.
impl LaneAdapter for NpmLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        crate::config::parse_npm_lane(label, fields)
    }

    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }

    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let mut report = LaneReport::new(name);
        let binary = self.binary.as_str();
        match runner.run_in(
            binary,
            &["update", "-g"],
            &Environment::inheriting().prepending_path(bin_dir(binary)),
            None,
        ) {
            // npm narrates its upgrades on stdout, but a week with nothing to
            // upgrade prints nothing at all, so this line is what says the lane
            // ran.
            Ok(_) => report.noted(format!("{binary} update -g: ok")),
            Err(why) => report.failed(format!("{binary} update -g FAILED ({why})")),
        }
        report
    }
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
    use crate::lanes::stubs::stub_facts;
    use std::cell::RefCell;
    use std::time::Duration;

    /// A runner that answers one scripted result and records every call it
    /// was asked to make. `Err` is the seam's own contract: an already
    /// composed reason, whether the command exited non-zero or could not be
    /// run at all.
    struct StubRunner {
        answer: Result<String, String>,
        calls: RefCell<Vec<Vec<String>>>,
        environments: RefCell<Vec<Environment>>,
    }

    impl StubRunner {
        fn clean() -> Self {
            StubRunner {
                answer: Ok(String::new()),
                calls: RefCell::new(Vec::new()),
                environments: RefCell::new(Vec::new()),
            }
        }

        fn refusing(why: &str) -> Self {
            StubRunner {
                answer: Err(why.to_string()),
                calls: RefCell::new(Vec::new()),
                environments: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        fn environments(&self) -> Vec<Environment> {
            self.environments.borrow().clone()
        }
    }

    impl CommandRunner for StubRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Result<String, String> {
            unreachable!("the npm lane runs its child in an environment of its own")
        }

        fn run_in(
            &self,
            program: &str,
            args: &[&str],
            env: &Environment,
            most: Option<Duration>,
        ) -> Result<String, String> {
            assert_eq!(most, None, "the whole lane owns the deadline");
            let mut call = vec![program.to_string()];
            call.extend(args.iter().map(|word| (*word).to_string()));
            self.calls.borrow_mut().push(call);
            self.environments.borrow_mut().push(env.clone());
            self.answer.clone()
        }

        fn run_with_deadline(
            &self,
            _program: &str,
            _args: &[&str],
            _most: Duration,
        ) -> Result<String, String> {
            unreachable!("the npm lane bounds no step of its own")
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
        lane().run("npm", &stub_facts(), &runner);
        assert_eq!(
            runner.calls(),
            vec![[NPM, "update", "-g"].map(String::from)]
        );
    }

    #[test]
    fn the_child_runs_with_npms_own_directory_ahead_of_the_inherited_path() {
        // The bug this exists for: npm on another node's PATH installs into
        // that node's prefix. The lane names the directory and the spawn seam
        // joins it to the inherited value; `prefixed_path` owns the join.
        let runner = StubRunner::clean();
        lane().run("npm", &stub_facts(), &runner);
        let env = runner.environments();
        assert_eq!(
            env,
            vec![Environment {
                variables: Default::default(),
                path_prefix: Some("/Users/someone/.local/share/fnm/aliases/default/bin".into()),
                only_these: false,
            }],
            "the fnm default bin directory goes first, over the inherited environment"
        );
    }

    #[test]
    fn no_shell_and_no_env_helper_stands_between_the_lane_and_npm() {
        let runner = StubRunner::clean();
        lane().run("npm", &stub_facts(), &runner);
        for call in runner.calls() {
            for word in &call {
                let program = std::path::Path::new(word).file_name();
                assert!(
                    !matches!(
                        program.and_then(|name| name.to_str()),
                        Some("sh") | Some("env")
                    ),
                    "{call:?}"
                );
            }
        }
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
    fn a_clean_upgrade_is_one_recorded_line_under_the_lanes_own_name() {
        // THE LANE'S OWN NAME, never the type's: `[lanes.globals]` with
        // `type = "npm"` is recorded and alerted as `globals`, and a report
        // carrying a hardcoded `npm` would name a lane nobody declared.
        let report = lane().run("globals", &stub_facts(), &StubRunner::clean());
        assert_eq!(report.name, "globals");
        assert_eq!(report.failures(), 0);
        assert_eq!(report.last_failure(), None);
        assert_eq!(report.lines, vec![format!("{NPM} update -g: ok")]);
    }

    #[test]
    fn an_upgrade_that_did_not_succeed_is_a_counted_failure_carrying_what_npm_said() {
        let report = lane().run(
            "npm",
            &stub_facts(),
            &StubRunner::refusing("exit 1: npm error code EACCES"),
        );
        assert_eq!(report.failures(), 1);
        let line = report.last_failure().expect("a failure names itself");
        assert!(line.contains("exit 1: npm error code EACCES"), "{line}");
        assert_eq!(report.lines, vec![line]);
    }

    #[test]
    fn an_npm_that_is_not_installed_is_a_failure_rather_than_a_quiet_skip() {
        // The bash job's own behavior, deliberately not ported: it printed
        // "npm is not at ...; nothing to upgrade" and returned 0.
        let report = lane().run(
            "npm",
            &stub_facts(),
            &StubRunner::refusing("exit 127: sh: npm: No such file or directory"),
        );
        assert_eq!(report.failures(), 1);
        assert!(
            report
                .last_failure()
                .is_some_and(|line| line.contains("No such file or directory")),
            "an absent npm must name itself in the record"
        );
    }
}
