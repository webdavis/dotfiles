//! `pns recap agent --stdin` and `pns recap git`: the two verbs an agent runs.
//!
//! THE ROUTE AND THE REPOSITORY ARE THE TWO THINGS NO UNIT TEST REACHES. The
//! domain tests pin the layout against facts handed to them; these pin the
//! facts and the wire.

mod support;

use std::process::Command;
use support::{Capture, Sandbox, plugin_command, run, stderr, stdout};

/// AN AGENT'S OWN RECAP ON THE SAME WIRE, and the same fallback behind it.
///
/// THE ROUTE IS THE WHOLE POINT OF THE COMMAND. The operator asked for these
/// recaps in `#pns-recap`, and that route is prepared in hermes separately from
/// pns, so the one thing this must never do is fail when it is absent. The
/// gateway is PROXIED rather than moved, for the reason the night recap's own
/// route test states: `PNS_HERMES_URL` outranks the route name, so an endpoint
/// override cannot observe the path.
#[test]
fn an_agent_recap_the_thread_route_will_not_take_falls_back_to_the_default_and_says_so() {
    let sandbox = Sandbox::new("recap-agent-fallback");
    sandbox.write_config("[plugins.hermes]\nenabled = true\nkey = \"gate-signing-key\"\n");
    let capture = Capture::start(&sandbox, "recap-agent-route", Some("404"), Some("2"));

    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_STATE_DIR", sandbox.path("state"))
        .env("HTTP_PROXY", capture.url())
        .env("http_proxy", capture.url());
    sandbox.stub_notifier(&mut command);
    let output = piped(
        command.args(["recap", "agent", "--stdin"]),
        b"Recap\n==========\n\n**User Tasks**\n1. `chezmoi apply`\n",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    let raw = capture.finish();
    let posted: Vec<&str> = raw
        .lines()
        .filter(|line| line.starts_with("POST /webhooks/"))
        .collect();
    assert_eq!(
        posted,
        [
            "POST /webhooks/pns-recap HTTP/1.1",
            "POST /webhooks/pns HTTP/1.1"
        ],
        "the recap route was tried first and the default caught it: {raw}"
    );
    let bodies: Vec<&str> = raw.split("\r\n\r\n").skip(1).collect();
    let fallback = bodies.last().expect("a second body");
    assert!(
        fallback.contains("did not take this"),
        "the fallback said nothing about why it landed here: {fallback}"
    );
    assert!(
        fallback.contains("User Tasks"),
        "the fallback carried a different body from the one it retried: {fallback}"
    );
}

#[test]
fn an_agent_recap_with_nothing_on_stdin_refuses_rather_than_posting_a_blank_message() {
    let sandbox = Sandbox::new("recap-agent-empty");
    let mut command = plugin_command(&sandbox);
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    sandbox.stub_notifier(&mut command);
    let output = piped(command.args(["recap", "agent", "--stdin"]), b"");
    assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("usage: pns recap agent --stdin"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_agent_recap_with_no_source_named_refuses_rather_than_reading_a_terminal() {
    let sandbox = Sandbox::new("recap-agent-no-source");
    let mut command = plugin_command(&sandbox);
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    sandbox.stub_notifier(&mut command);
    let output = piped(command.args(["recap", "agent"]), b"whatever\n");
    assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
}

/// THE COMMAND THE SKILL RUNS, IN A REAL REPOSITORY. It is the only assertion
/// that the git reads, the ordering and the rendering agree: the domain tests
/// pin the layout against facts handed to them, and this pins the facts.
#[test]
fn recap_git_reads_the_branch_the_worktree_and_the_diff_out_of_a_real_repository() {
    let sandbox = Sandbox::new("recap-git");
    let repository = sandbox.path("repo").display().to_string();
    std::fs::create_dir_all(&repository).expect("the repository directory");
    let git = |arguments: &[&str]| {
        let status = Command::new("git")
            .args(["-C", &repository])
            .args(arguments)
            // GIT EXPORTS ITS OWN DIR TO EVERY HOOK, and this suite runs under
            // one, so a stray `GIT_DIR` would silently no-op the init.
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {arguments:?}");
    };
    git(&["init", "--initial-branch", "main", "--quiet"]);
    std::fs::write(format!("{repository}/kept.txt"), "one\n").expect("a tracked file");
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "root"]);
    // THE TRUNK IS A REMOTE-TRACKING REF, which is what `origin/main...HEAD`
    // reads and what a clone has. There is no remote to fetch from, so the ref
    // is written directly.
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["checkout", "--quiet", "-b", "feat/example"]);
    std::fs::write(format!("{repository}/added.txt"), "two\n").expect("a new file");
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "work"]);

    let mut command = plugin_command(&sandbox);
    command
        .current_dir(&repository)
        .env("PNS_STATE_DIR", sandbox.path("state"))
        // NO NETWORK: an empty PATH addition is not how this is
        // done, so `gh` simply is not reachable and the PR line says so,
        // which is the branch this asserts.
        .env("PATH", no_listing_path());
    let output = run(command.args(["recap", "git"]));
    let printed = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        printed.contains("- Branch: `feat/example` (PR unknown, 1 of 1 in stack)"),
        "{printed}"
    );
    assert!(
        printed.contains("- PR: unknown (gh did not answer)"),
        "{printed}"
    );
    assert!(
        printed.contains("- Stack: `feat/example` (0 PRs, trunk main)"),
        "{printed}"
    );
    assert!(printed.contains("  `main` (*trunk*)"), "{printed}");
    assert!(
        printed.contains("  └─ `feat/example`  (PR unknown)  ×  ← *current*"),
        "{printed}"
    );
    assert!(printed.contains("A  added.txt"), "{printed}");
    assert!(!printed.contains("kept.txt"), "{printed}");
}

/// A PATH with git on it and no `gh`, so the pull-request listing is the one
/// thing that cannot run.
fn no_listing_path() -> String {
    let git = Command::new("/usr/bin/env")
        .args(["sh", "-c", "command -v git"])
        .output()
        .expect("git is on PATH");
    let git = String::from_utf8_lossy(&git.stdout).trim().to_string();
    std::path::Path::new(&git)
        .parent()
        .expect("git's own directory")
        .display()
        .to_string()
}

/// One run with `payload` on stdin.
fn piped(command: &mut Command, payload: &[u8]) -> std::process::Output {
    use std::io::Write as _;
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    let mut stdin = child.stdin.take().expect("stdin");
    let _ = stdin.write_all(payload);
    drop(stdin);
    child.wait_with_output().expect("the engine exits")
}
