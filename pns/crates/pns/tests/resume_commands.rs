//! `pns resume`: the "where was I" answer, through the real binary.
//!
//! THE HERDR LISTING AND THE STORED STATE ARE WHAT NO UNIT TEST REACHES. The
//! unit tests beside the command pin the layout against facts handed to them;
//! these pin where those facts come from, and that `--notify` reaches the
//! ordinary producer path rather than a delivery of its own.

#[path = "support/stored_records.rs"]
mod stored_records;
mod support;

use pns_adapters::{SessionNote, SqliteStore};
use std::process::Command;
use support::{Sandbox, run, run_expecting, stderr, stdout};

/// Two workspaces, one focused, the other opened on the waiting branch's own
/// worktree, which is the arrangement every lane of this repository runs in.
const LISTED: &str = concat!(
    r#"{"result":{"workspaces":["#,
    r#"{"label":"dotfiles modernization","focused":true,"#,
    r#" "worktree":{"checkout_path":"/repo/dotfiles"}},"#,
    r#"{"label":"lane","focused":false,"#,
    r#" "worktree":{"checkout_path":"/worktrees/dotfiles/feat-resume"}}]}}"#
);

fn stub_workspaces(sandbox: &Sandbox, command: &mut Command) {
    sandbox.stub_on_path(command, "herdr", &format!("printf '%s' '{LISTED}'"));
}

/// One session named, then blocked, through the store's own API. The live
/// store is never opened: `PNS_STATE_DIR` and this path are the sandbox's.
fn seed_waiting_session(sandbox: &Sandbox) {
    let store = SqliteStore::for_records(sandbox.state());
    store
        .note_session(&SessionNote {
            id: "s1",
            harness: "claude",
            project: "dotfiles",
            branch: "feat/resume",
            title: "ship the resume page",
            now: 1_000,
        })
        .expect("the session is recorded");
    store
        .begin_wait("s1", 1_100, true)
        .expect("the wait is recorded");
}

/// One timed shell command, submitted the way the notifier submits one.
fn seed_shell_command(sandbox: &Sandbox, detail: &str) {
    let mut command = sandbox.pns_stateful();
    stub_workspaces(sandbox, &mut command);
    let output = run(command.args([
        "send",
        "--producer",
        "shell",
        "--state",
        "done",
        "--detail",
        detail,
    ]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

fn resume(sandbox: &Sandbox, flags: &[&str]) -> std::process::Output {
    let mut command = sandbox.pns_stateful();
    stub_workspaces(sandbox, &mut command);
    run(command.arg("resume").args(flags))
}

#[test]
fn the_page_names_the_focused_workspace_the_waiting_session_and_the_last_command() {
    let sandbox = Sandbox::new("resume-page");
    seed_waiting_session(&sandbox);
    seed_shell_command(&sandbox, "cargo");

    let output = resume(&sandbox, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let printed = stdout(&output);
    for fact in [
        "dotfiles modernization",
        "ship the resume page",
        "feat/resume",
        "/worktrees/dotfiles/feat-resume",
        "cargo",
    ] {
        assert!(printed.contains(fact), "{fact} missing from {printed}");
    }
}

#[test]
fn a_machine_with_nothing_waiting_says_so_rather_than_printing_an_empty_section() {
    let sandbox = Sandbox::new("resume-nothing-waiting");
    let output = resume(&sandbox, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let printed = stdout(&output);
    assert!(printed.contains("Nothing is waiting on you."), "{printed}");
    assert!(
        printed.contains("dotfiles modernization"),
        "the workspace is still answered: {printed}"
    );
}

#[test]
fn the_json_form_carries_the_same_answers_under_the_page_s_own_names() {
    let sandbox = Sandbox::new("resume-json");
    seed_waiting_session(&sandbox);
    seed_shell_command(&sandbox, "just");

    let output = resume(&sandbox, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let encoded = stdout(&output);
    let page: serde_json::Value = serde_json::from_str(encoded.trim()).expect("one JSON object");
    for (field, expected) in [
        ("workspace", "dotfiles modernization"),
        ("title", "ship the resume page"),
        ("branch", "feat/resume"),
        ("worktree", "/worktrees/dotfiles/feat-resume"),
        ("command", "just"),
    ] {
        assert_eq!(page[field], expected, "{field} in {encoded}");
    }
    assert_eq!(page["waiting"], true, "{encoded}");
}

#[test]
fn notify_records_one_event_from_pns_whose_message_is_the_page() {
    let sandbox = Sandbox::new("resume-notify");
    seed_waiting_session(&sandbox);

    let mut command = sandbox.pns_stateful();
    stub_workspaces(&sandbox, &mut command);
    let output = run(command.args(["resume", "--notify"]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    let recorded: Vec<(String, String)> = stored_records::database(&sandbox)
        .prepare("SELECT producer, message FROM ledger_events ORDER BY seq")
        .expect("the ledger rows")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("read the recorded events")
        .collect::<rusqlite::Result<_>>()
        .expect("recorded events");
    assert_eq!(recorded.len(), 1, "one page, one event: {recorded:?}");
    let (producer, message) = &recorded[0];
    assert_eq!(producer, "pns");
    for fact in [
        "pns resume",
        "dotfiles modernization",
        "ship the resume page",
        "/worktrees/dotfiles/feat-resume",
    ] {
        assert!(message.contains(fact), "{fact} missing from {message}");
    }
    assert!(
        !message.contains('\u{1b}'),
        "the delivered page carries an escape sequence: {message}"
    );
}

#[test]
fn resume_answers_its_own_help_and_refuses_a_flag_it_does_not_take() {
    let sandbox = Sandbox::new("resume-usage");
    for spelling in ["--help", "-h"] {
        let output = run(sandbox.pns().args(["resume", spelling]));
        assert_eq!(output.status.code(), Some(0), "{spelling}: {output:?}");
        assert!(
            stdout(&output).contains("pns resume [--json | --notify]"),
            "{spelling}: {}",
            stdout(&output)
        );
    }
    let output = run_expecting(2, sandbox.pns().args(["resume", "--json", "--notify"]));
    assert!(stderr(&output).contains("pns resume"), "{output:?}");
}
