use super::*;
use crate::lanes::Ran;
use crate::lanes::stubs::stub_facts;
use std::cell::RefCell;
use std::time::Duration;

struct StubRustup {
    answer: Result<String, String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl StubRustup {
    fn saying(stdout: &str) -> Self {
        StubRustup {
            answer: Ok(stdout.to_string()),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn refusing(why: &str) -> Self {
        StubRustup {
            answer: Err(why.to_string()),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for StubRustup {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|word| (*word).to_string()));
        self.calls.borrow_mut().push(call);
        self.answer.clone()
    }

    fn run_with_deadline(
        &self,
        _program: &str,
        _args: &[&str],
        _most: Duration,
    ) -> Result<String, String> {
        unreachable!("the rustup lane bounds no step of its own")
    }

    fn run_with_input(&self, _program: &str, _args: &[&str], _input: &str) -> Result<Ran, String> {
        unreachable!("the rustup lane hands its child nothing on stdin")
    }
}

const SUMMARY: &str = "\
  stable-aarch64-apple-darwin unchanged - rustc 1.98.1 (48a229cea 2026-09-01)
   nightly-aarch64-apple-darwin updated - rustc 1.100.0-nightly (a36d05efa 2026-09-09) (from rustc 1.92.0-nightly (0be8e1608 2025-09-19))
";

fn lane() -> RustupLane {
    RustupLane {
        rustup: "/Users/x/.cargo/bin/rustup".to_string(),
    }
}

#[test]
fn the_lane_updates_every_toolchain_with_one_call_to_the_declared_binary() {
    let runner = StubRustup::saying(SUMMARY);
    lane().run("rustup", &stub_facts(), &runner);
    assert_eq!(
        runner.calls(),
        vec![
            ["/Users/x/.cargo/bin/rustup", "update"]
                .map(String::from)
                .to_vec()
        ]
    );
}

#[test]
fn each_toolchain_is_one_recorded_line_saying_what_became_of_it() {
    // THE SUMMARY IS READ, not just the exit code: rustup exits 0 whether it
    // moved a toolchain or found nothing to do, so a record of "ok" could not
    // answer the question the lane exists for.
    let report = lane().run("rustup", &stub_facts(), &StubRustup::saying(SUMMARY));
    assert_eq!(report.failures(), 0);
    assert_eq!(
        report.lines,
        vec![
            "stable-aarch64-apple-darwin: rustc 1.98.1 (48a229cea 2026-09-01), current",
            "nightly-aarch64-apple-darwin: rustc 1.92.0-nightly (0be8e1608 2025-09-19) → rustc 1.100.0-nightly (a36d05efa 2026-09-09)",
        ]
    );
}

#[test]
fn a_rustup_that_fails_is_a_failure_carrying_its_stderr_tail() {
    let report = lane().run(
        "rustup",
        &stub_facts(),
        &StubRustup::refusing("exit 1: error: could not download file"),
    );
    assert_eq!(report.failures(), 1);
    assert!(
        report
            .last_failure()
            .is_some_and(|line| line.contains("could not download file"))
    );
}

#[test]
fn a_run_that_named_no_toolchain_says_so_rather_than_claiming_everything_current() {
    // A silent success and a summary nothing could read look identical in the
    // record otherwise, and only one of them means the toolchains are fine.
    let report = lane().run(
        "rustup",
        &stub_facts(),
        &StubRustup::saying("info: cleaning up downloads & tmp directories\n"),
    );
    assert_eq!(report.failures(), 0);
    assert_eq!(report.lines.len(), 1);
    assert!(
        report.lines[0].contains("named no toolchain"),
        "{:?}",
        report.lines
    );
}

#[test]
fn the_lane_is_recorded_under_its_own_name_and_not_the_types() {
    let report = lane().run("toolchains", &stub_facts(), &StubRustup::saying(SUMMARY));
    assert_eq!(report.name, "toolchains");
}
