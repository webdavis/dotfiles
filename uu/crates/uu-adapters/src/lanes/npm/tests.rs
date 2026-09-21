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
    listing: Result<String, String>,
    calls: RefCell<Vec<Vec<String>>>,
    environments: RefCell<Vec<Environment>>,
}

impl StubRunner {
    fn clean() -> Self {
        StubRunner {
            answer: Ok(String::new()),
            listing: Ok(LISTED.to_string()),
            calls: RefCell::new(Vec::new()),
            environments: RefCell::new(Vec::new()),
        }
    }

    /// A clean upgrade over a listing of this stub's own.
    fn listing(answer: Result<String, String>) -> Self {
        StubRunner {
            listing: answer,
            ..StubRunner::clean()
        }
    }

    fn refusing(why: &str) -> Self {
        StubRunner {
            answer: Err(why.to_string()),
            listing: Ok(LISTED.to_string()),
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
        if args == LISTING {
            return self.listing.clone();
        }
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

    fn run_with_input(&self, _program: &str, _args: &[&str], _input: &str) -> Result<Ran, String> {
        unreachable!("the npm lane hands its child nothing on stdin")
    }
}

/// `npm ls -g --depth=0 --json` as it answered on this machine, with the
/// package names replaced. Both of node's own bundled packages are kept as
/// they appeared, because excluding them is what the fixture proves.
const LISTED: &str = r#"{
  "name": "lib",
  "dependencies": {
    "@scope/one": { "version": "6.0.3", "overridden": false },
    "corepack": { "version": "0.36.0", "overridden": false },
    "npm": { "version": "12.0.2", "overridden": false },
    "two": { "version": "0.18.0", "overridden": false },
    "three": { "version": "1.2.4", "overridden": false }
  }
}"#;

/// The shipped configuration's own npm: fnm's version-free bin dir.
const NPM: &str = "/Users/someone/.local/share/fnm/aliases/default/bin/npm";

fn lane() -> NpmLane {
    NpmLane {
        binary: NPM.to_string(),
        declared: None,
    }
}

/// The same lane with a roster to measure the listing against.
fn declaring(declared: &[&str]) -> NpmLane {
    NpmLane {
        declared: Some(declared.iter().map(|name| (*name).to_string()).collect()),
        ..lane()
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

#[test]
fn a_lane_with_no_declared_roster_lists_nothing_and_reports_only_the_upgrade() {
    let runner = StubRunner::clean();
    let report = lane().run("npm", &stub_facts(), &runner);
    assert_eq!(
        runner.calls(),
        vec![[NPM, "update", "-g"].map(String::from)],
        "an absent roster leaves the lane exactly as it was"
    );
    assert_eq!(report.lines, vec![format!("{NPM} update -g: ok")]);
}

#[test]
fn a_declared_roster_has_the_listing_read_with_the_same_npm_and_path() {
    let runner = StubRunner::clean();
    declaring(&[]).run("npm", &stub_facts(), &runner);
    assert_eq!(
        runner.calls(),
        vec![
            [NPM, "update", "-g"].map(String::from).to_vec(),
            [NPM, "ls", "-g", "--depth=0", "--json"]
                .map(String::from)
                .to_vec(),
        ]
    );
    for env in runner.environments() {
        assert_eq!(
            env.path_prefix.as_deref(),
            Some("/Users/someone/.local/share/fnm/aliases/default/bin"),
            "the listing has to come from the same node's prefix as the upgrade"
        );
    }
}

#[test]
fn every_installed_package_the_roster_does_not_name_is_one_noted_line() {
    let report =
        declaring(&["@scope/one", "two"]).run("globals", &stub_facts(), &StubRunner::clean());
    assert_eq!(report.failures(), 0);
    assert_eq!(
        report.lines,
        vec![
            format!("{NPM} update -g: ok"),
            "undeclared: three 1.2.4".to_string(),
        ]
    );
}

#[test]
fn node_s_own_bundled_packages_are_never_reported() {
    // `npm` and `corepack` come with node, so no roster declares them and
    // reporting them would be two permanent lines every week.
    let report =
        declaring(&["@scope/one", "three", "two"]).run("npm", &stub_facts(), &StubRunner::clean());
    assert_eq!(report.lines, vec![format!("{NPM} update -g: ok")]);
}

#[test]
fn a_listing_that_could_not_be_run_is_a_failed_step_carrying_what_npm_said() {
    let report = declaring(&[]).run(
        "npm",
        &stub_facts(),
        &StubRunner::listing(Err("exit 1: npm error code EACCES".to_string())),
    );
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("exit 1: npm error code EACCES")),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_listing_that_is_not_readable_json_is_a_failed_step_rather_than_a_guess() {
    // Nothing undeclared and an unreadable answer must not read the same.
    let report = declaring(&[]).run(
        "npm",
        &stub_facts(),
        &StubRunner::listing(Ok("up to date in 0.4s".to_string())),
    );
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("unreadably")),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_listing_with_no_dependencies_reports_nothing_and_fails_nothing() {
    let report = declaring(&[]).run(
        "npm",
        &stub_facts(),
        &StubRunner::listing(Ok("{ \"name\": \"lib\" }".to_string())),
    );
    assert_eq!(report.failures(), 0);
    assert_eq!(report.lines, vec![format!("{NPM} update -g: ok")]);
}
