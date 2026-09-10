use super::*;
use crate::lanes::Ran;
use crate::lanes::stubs::stub_facts;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

/// A runner that answers per argv and records every call, because this lane
/// makes one call for the listing and then one per crate.
struct ScriptedCargo {
    answers: HashMap<String, Result<String, String>>,
    fallback: Result<String, String>,
    calls: RefCell<Vec<String>>,
}

impl ScriptedCargo {
    fn new(answers: &[(&str, Result<&str, &str>)]) -> Self {
        ScriptedCargo {
            answers: answers
                .iter()
                .map(|(argv, answer)| {
                    (
                        (*argv).to_string(),
                        answer.map(str::to_string).map_err(str::to_string),
                    )
                })
                .collect(),
            fallback: Ok(String::new()),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for ScriptedCargo {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let argv = format!("{program} {}", args.join(" "));
        self.calls.borrow_mut().push(argv.clone());
        self.answers.get(&argv).cloned().unwrap_or_else(|| {
            let _ = program;
            self.fallback.clone()
        })
    }

    fn run_with_deadline(
        &self,
        _program: &str,
        _args: &[&str],
        _most: Duration,
    ) -> Result<String, String> {
        unreachable!("the cargo lane bounds no step of its own")
    }

    fn run_with_input(&self, _program: &str, _args: &[&str], _input: &str) -> Result<Ran, String> {
        unreachable!("the cargo lane hands its child nothing on stdin")
    }
}

const CARGO: &str = "/Users/x/.cargo/bin/cargo";

fn lane(compile: bool) -> CargoLane {
    CargoLane {
        cargo: CARGO.to_string(),
        compile,
    }
}

/// One registry crate, one git crate.
const LIST: &str = "\
fd-find v8.4.0:
    fd
herdr-navigator v0.1.0 (https://example.git?rev=deed8496356aab90e1cc364dac4f95d898fa6067#deed8496):
    herdr-navigator
";

fn listing() -> (&'static str, Result<&'static str, &'static str>) {
    ("/Users/x/.cargo/bin/cargo install --list", Ok(LIST))
}

fn search(name: &str) -> String {
    format!("{CARGO} search {name} --limit 1")
}

#[test]
fn a_crate_that_is_behind_makes_the_lane_pending_when_compile_is_off() {
    let runner = ScriptedCargo::new(&[
        listing(),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Ok("fd-find = \"10.5.0\"    # a find alternative\n"),
        ),
    ]);
    let report = lane(false).run("cargo", &stub_facts(), &runner);
    assert_eq!(report.verdict(), uu_domain::LaneVerdict::Pending);
    assert_eq!(report.failures(), 0);
    assert!(
        report.lines.iter().any(|line| line
            == "fd has a new version: 8.4.0 → 10.5.0. \
                Run the following command to compile it: cargo install fd-find"),
        "{:?}",
        report.lines
    );
    // NOT COMPILED. The whole point of the default is that nothing is built.
    assert!(
        !runner
            .calls()
            .iter()
            .any(|call| call == &format!("{CARGO} install fd-find")),
        "{:?}",
        runner.calls()
    );
}

#[test]
fn a_crate_that_is_current_is_one_recorded_line() {
    let runner = ScriptedCargo::new(&[
        listing(),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Ok("fd-find = \"8.4.0\"    # a find alternative\n"),
        ),
    ]);
    let report = lane(false).run("cargo", &stub_facts(), &runner);
    assert_eq!(report.failures(), 0);
    assert!(
        report.lines.contains(&"fd-find 8.4.0: current".to_string()),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_git_sourced_crate_is_named_and_never_searched() {
    let runner = ScriptedCargo::new(&[listing()]);
    let report = lane(false).run("cargo", &stub_facts(), &runner);
    assert!(
        report
            .lines
            .iter()
            .any(|line| line.contains("herdr-navigator") && line.contains("deed8496")),
        "{:?}",
        report.lines
    );
    assert!(
        !runner.calls().contains(&search("herdr-navigator")),
        "the registry cannot answer for a git install, so it must not be asked"
    );
}

#[test]
fn a_search_that_fails_is_a_failure_naming_the_crate_and_the_rest_still_run() {
    // Two registry crates, the first refusing.
    let two = "fd-find v8.4.0:\n    fd\nselene v0.26.1:\n    selene\n";
    let runner = ScriptedCargo::new(&[
        ("/Users/x/.cargo/bin/cargo install --list", Ok(two)),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Err("exit 1: network unreachable"),
        ),
        (
            "/Users/x/.cargo/bin/cargo search selene --limit 1",
            Ok("selene = \"0.26.1\"    # a lua linter\n"),
        ),
    ]);
    let report = lane(false).run("cargo", &stub_facts(), &runner);
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("fd-find") && line.contains("network unreachable"))
    );
    assert!(
        report.lines.contains(&"selene 0.26.1: current".to_string()),
        "one crate's failure must not hide the next: {:?}",
        report.lines
    );
}

#[test]
fn a_listing_that_cannot_be_read_is_one_failure_and_no_crate_is_claimed_current() {
    let runner = ScriptedCargo::new(&[(
        "/Users/x/.cargo/bin/cargo install --list",
        Err("could not run cargo: No such file or directory"),
    )]);
    let report = lane(false).run("cargo", &stub_facts(), &runner);
    assert_eq!(report.failures(), 1);
    assert_eq!(report.lines.len(), 1);
    assert_eq!(
        runner.calls().len(),
        1,
        "nothing is searched without a list"
    );
}

#[test]
fn a_crate_the_registry_names_no_version_for_is_recorded_rather_than_called_current() {
    let runner = ScriptedCargo::new(&[
        listing(),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Ok("note: to learn more, run `cargo info`\n"),
        ),
    ]);
    let report = lane(false).run("cargo", &stub_facts(), &runner);
    assert_eq!(report.failures(), 0);
    assert!(
        report
            .lines
            .iter()
            .any(|line| line.contains("named no version")),
        "{:?}",
        report.lines
    );
}

#[test]
fn the_lane_is_recorded_under_its_own_name_and_not_the_types() {
    // `[lanes.rust-tools]` with `type = "cargo"` is recorded as `rust-tools`.
    let report = lane(false).run(
        "rust-tools",
        &stub_facts(),
        &ScriptedCargo::new(&[listing()]),
    );
    assert_eq!(report.name, "rust-tools");
}

// --- compile = true -------------------------------------------------------

#[test]
fn with_compile_on_a_crate_behind_is_installed_by_name_and_recorded_with_both_versions() {
    let runner = ScriptedCargo::new(&[
        listing(),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Ok("fd-find = \"10.5.0\"    # a find alternative\n"),
        ),
    ]);
    let report = lane(true).run("cargo", &stub_facts(), &runner);
    // INSTALLED BY CRATE NAME, never by binary: `cargo install fd` is a
    // different crate that may not exist.
    assert!(
        runner.calls().contains(&format!("{CARGO} install fd-find")),
        "{:?}",
        runner.calls()
    );
    assert!(
        report
            .lines
            .contains(&"fd-find: compiled 8.4.0 → 10.5.0".to_string()),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_compiled_run_is_completed_rather_than_pending() {
    // Pending means "there is something left for you to do". Once the lane has
    // done it, saying so would leave the operator looking for work that is done.
    let runner = ScriptedCargo::new(&[
        listing(),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Ok("fd-find = \"10.5.0\"    # a find alternative\n"),
        ),
    ]);
    let report = lane(true).run("cargo", &stub_facts(), &runner);
    assert_ne!(report.verdict(), uu_domain::LaneVerdict::Pending);
    assert_eq!(report.failures(), 0);
}

#[test]
fn one_failed_build_does_not_stop_the_next_crate() {
    let two = "fd-find v8.4.0:\n    fd\nselene v0.26.1:\n    selene\n";
    let runner = ScriptedCargo::new(&[
        ("/Users/x/.cargo/bin/cargo install --list", Ok(two)),
        (
            "/Users/x/.cargo/bin/cargo search fd-find --limit 1",
            Ok("fd-find = \"10.5.0\"    # a find alternative\n"),
        ),
        (
            "/Users/x/.cargo/bin/cargo install fd-find",
            Err("exit 101: error: linker `cc` not found"),
        ),
        (
            "/Users/x/.cargo/bin/cargo search selene --limit 1",
            Ok("selene = \"0.31.0\"    # a lua linter\n"),
        ),
    ]);
    let report = lane(true).run("cargo", &stub_facts(), &runner);
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("fd-find") && line.contains("linker `cc` not found"))
    );
    assert!(
        runner.calls().contains(&format!("{CARGO} install selene")),
        "a crate that will not build must not hide the next: {:?}",
        runner.calls()
    );
}
