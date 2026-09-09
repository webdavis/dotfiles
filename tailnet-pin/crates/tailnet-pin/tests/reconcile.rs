//! The compiled binary, run against a hosts file that is not the machine's.
//!
//! THE SEAM IS WHAT MAKES THIS SAFE TO RUN. `TAILNET_PIN_HOSTS_FILE` points the
//! whole tool at a scratch file, and the one test that leaves it unset asserts a
//! refusal that happens before any file is opened.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LOCALHOST: &str = "127.0.0.1\tlocalhost\n";
const RECORD: &str = "10.0.0.5\tpin.example.test\tpin\n";

/// A private directory that removes itself.
struct Scratchpad(PathBuf);

impl Scratchpad {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("tailnet-pin-cli-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("a scratch directory");
        Scratchpad(root)
    }

    fn hosts(&self, contents: &str) -> PathBuf {
        let path = self.0.join("hosts");
        fs::write(&path, contents).expect("a scratch hosts file");
        path
    }
}

impl Drop for Scratchpad {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_against(hosts: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tailnet-pin"))
        .env("TAILNET_PIN_HOSTS_FILE", hosts)
        .args(arguments)
        .output()
        .expect("the binary runs")
}

fn pin_arguments() -> [&'static str; 3] {
    ["pin.example.test", "10.0.0.5", "pin"]
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The ordinary first run, end to end through the real file.
#[test]
fn a_first_run_writes_the_record_and_says_so() {
    let scratch = Scratchpad::new("first-run");
    let hosts = scratch.hosts(LOCALHOST);
    let output = run_against(&hosts, &pin_arguments());

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(&hosts).expect("read back"),
        format!("{LOCALHOST}{RECORD}")
    );
    assert!(
        stdout(&output).contains("written to"),
        "{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains("pin.example.test"),
        "{}",
        stdout(&output)
    );
}

/// A second run changes nothing, which is what makes this safe to run on every
/// apply.
#[test]
fn a_second_run_changes_nothing_and_says_so() {
    let scratch = Scratchpad::new("converged");
    let hosts = scratch.hosts(&format!("{LOCALHOST}{RECORD}"));
    let before = fs::metadata(&hosts)
        .expect("metadata")
        .modified()
        .expect("mtime");

    let output = run_against(&hosts, &pin_arguments());
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("already converged"),
        "{}",
        stdout(&output)
    );
    assert_eq!(
        fs::metadata(&hosts)
            .expect("metadata")
            .modified()
            .expect("mtime"),
        before,
        "a converged file was rewritten"
    );
}

/// THE GATE, end to end: nothing is installed and the message says what was
/// lost, because a hosts file with no localhost is how a machine stops
/// resolving its own name.
#[test]
fn a_rebuild_that_would_lose_localhost_refuses_and_leaves_the_file_alone() {
    let scratch = Scratchpad::new("gate");
    let source = "127.0.0.1\tlocalhost\tpin\n";
    let hosts = scratch.hosts(source);

    let output = run_against(&hosts, &pin_arguments());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("loopback"), "{}", stderr(&output));
    assert_eq!(fs::read_to_string(&hosts).expect("read back"), source);
}

/// A field that is not one hosts column would write extra columns, or extra
/// LINES, into a file being rewritten as root.
#[test]
fn a_field_that_is_not_one_column_is_refused_by_name() {
    let scratch = Scratchpad::new("column");
    let hosts = scratch.hosts(LOCALHOST);

    let output = run_against(&hosts, &["pin.example.test", "10.0.0.5", "pin two"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("short"), "{}", stderr(&output));
    assert_eq!(fs::read_to_string(&hosts).expect("read back"), LOCALHOST);
}

/// Argv this does not serve exits 2 and touches nothing, which is a different
/// answer from a refusal so a caller can tell them apart.
#[test]
fn the_wrong_number_of_arguments_is_usage() {
    let scratch = Scratchpad::new("usage");
    let hosts = scratch.hosts(LOCALHOST);

    for arguments in [vec![], vec!["pin.example.test"], vec!["a", "b", "c", "d"]] {
        let output = run_against(&hosts, &arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(stderr(&output).starts_with("usage:"), "{}", stderr(&output));
    }
    assert_eq!(fs::read_to_string(&hosts).expect("read back"), LOCALHOST);
}

/// SET BUT EMPTY IS NOT UNSET. Reading the two the same way is what aims a root
/// rewrite at the real /etc/hosts when a caller produced an empty path.
#[test]
fn an_empty_seam_variable_refuses_rather_than_falling_back_to_the_real_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_tailnet-pin"))
        .env("TAILNET_PIN_HOSTS_FILE", "")
        .args(pin_arguments())
        .output()
        .expect("the binary runs");

    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("set but EMPTY"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("/etc/hosts"),
        "{}",
        stderr(&output)
    );
}

/// A path naming no file refuses rather than creating one: this tool converges
/// an existing hosts file and never invents one.
#[test]
fn a_missing_file_is_refused_rather_than_created() {
    let scratch = Scratchpad::new("missing");
    let absent = scratch.0.join("nowhere");

    let output = run_against(&absent, &pin_arguments());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(!absent.exists(), "a hosts file was invented");
}

/// A stale pin is the case the tool exists for, and nothing is left beside the
/// file when it is done.
#[test]
fn a_stale_pin_is_replaced_and_no_scratch_file_is_left_behind() {
    let scratch = Scratchpad::new("stale");
    let hosts = scratch.hosts(&format!("{LOCALHOST}10.9.9.9\tpin.example.test\tpin\n"));

    let output = run_against(&hosts, &pin_arguments());
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        fs::read_to_string(&hosts).expect("read back"),
        format!("{LOCALHOST}{RECORD}")
    );

    let left: Vec<_> = fs::read_dir(&scratch.0)
        .expect("listing")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
}
