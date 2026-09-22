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
}

impl StubRunner {
    fn clean() -> Self {
        StubRunner {
            answer: Ok(String::new()),
            listing: Ok(LISTED.to_string()),
            calls: RefCell::new(Vec::new()),
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
        unreachable!("the uv lane bounds no step of its own")
    }

    fn run_with_input(&self, _program: &str, _args: &[&str], _input: &str) -> Result<Ran, String> {
        unreachable!("the uv lane hands its child nothing on stdin")
    }
}

/// `uv tool list` as it answered on this machine, with the tool names
/// replaced. Every executable line is kept, because skipping them is what
/// the fixture proves.
const LISTED: &str = "one v0.9.53\n- one\n- one-mcp\ntwo v0.2.1\n- two\nthree v11.3.1\n- three\n";

fn lane() -> UvLane {
    UvLane {
        binary: "/opt/homebrew/bin/uv".to_string(),
        declared: None,
    }
}

/// The same lane with a roster to measure the listing against.
fn declaring(declared: &[&str]) -> UvLane {
    UvLane {
        declared: Some(declared.iter().map(|name| (*name).to_string()).collect()),
        ..lane()
    }
}

#[test]
fn a_lane_with_no_declared_roster_lists_nothing_and_reports_only_the_upgrade() {
    let runner = StubRunner::clean();
    let report = lane().run("uv", &stub_facts(), &runner);
    assert_eq!(
        runner.calls(),
        vec![
            ["/opt/homebrew/bin/uv", "tool", "upgrade", "--all"]
                .map(String::from)
                .to_vec()
        ],
        "an absent roster leaves the lane exactly as it was"
    );
    assert_eq!(
        report.lines,
        vec!["/opt/homebrew/bin/uv tool upgrade --all: ok"]
    );
}

#[test]
fn a_declared_roster_has_the_listing_read_after_the_upgrade() {
    let runner = StubRunner::clean();
    declaring(&[]).run("uv", &stub_facts(), &runner);
    assert_eq!(
        runner.calls(),
        vec![
            ["/opt/homebrew/bin/uv", "tool", "upgrade", "--all"]
                .map(String::from)
                .to_vec(),
            ["/opt/homebrew/bin/uv", "tool", "list"]
                .map(String::from)
                .to_vec(),
        ]
    );
}

#[test]
fn every_installed_tool_the_roster_does_not_name_is_one_noted_line() {
    // The executables each tool installed are entries under it, never
    // tools of their own, so none of them is ever reported.
    let report = declaring(&["one", "three"]).run("tools", &stub_facts(), &StubRunner::clean());
    assert_eq!(report.failures(), 0);
    assert_eq!(
        report.lines,
        vec![
            "/opt/homebrew/bin/uv tool upgrade --all: ok".to_string(),
            "undeclared: two 0.2.1".to_string(),
        ]
    );
}

#[test]
fn a_fully_declared_machine_records_the_upgrade_and_nothing_else() {
    let report = declaring(&["one", "two", "three"]).run("uv", &stub_facts(), &StubRunner::clean());
    assert_eq!(
        report.lines,
        vec!["/opt/homebrew/bin/uv tool upgrade --all: ok"]
    );
}

#[test]
fn a_listing_that_could_not_be_run_is_a_failed_step_carrying_what_uv_said() {
    let report = declaring(&[]).run(
        "uv",
        &stub_facts(),
        &StubRunner::listing(Err("exit 2: error: unrecognized subcommand".to_string())),
    );
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("unrecognized subcommand")),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_listing_holding_no_tool_line_reports_nothing_and_fails_nothing() {
    let report = declaring(&[]).run(
        "uv",
        &stub_facts(),
        &StubRunner::listing(Ok("No tools installed.\n".to_string())),
    );
    assert_eq!(report.failures(), 0);
    assert_eq!(
        report.lines,
        vec!["/opt/homebrew/bin/uv tool upgrade --all: ok"]
    );
}

#[test]
fn the_lane_upgrades_every_uv_tool_with_one_call_to_the_declared_binary() {
    let runner = StubRunner::clean();
    lane().run("uv", &stub_facts(), &runner);
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
    let report = lane().run("tools", &stub_facts(), &StubRunner::clean());
    assert_eq!(report.name, "tools");
    assert_eq!(report.failures(), 0);
    assert_eq!(report.last_failure(), None);
    assert_eq!(
        report.lines,
        vec!["/opt/homebrew/bin/uv tool upgrade --all: ok"]
    );
}

#[test]
fn an_upgrade_that_did_not_succeed_is_a_counted_failure_carrying_what_uv_said() {
    let report = lane().run(
        "uv",
        &stub_facts(),
        &StubRunner::refusing("exit 2: error: no such option `--all`"),
    );
    assert_eq!(report.failures(), 1);
    let line = report.last_failure().expect("a failure names itself");
    assert!(line.contains("exit 2: error: no such option"), "{line}");
    assert_eq!(report.lines, vec![line]);
}

#[test]
fn a_uv_that_is_not_installed_is_a_failure_rather_than_a_quiet_skip() {
    // The bash job's own behavior, deliberately not ported: it printed
    // "uv is not at ...; nothing to upgrade" and returned 0.
    let report = lane().run(
        "uv",
        &stub_facts(),
        &StubRunner::refusing("could not run /opt/homebrew/bin/uv: No such file or directory"),
    );
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("No such file or directory")),
        "an absent uv must name itself in the record"
    );
}
