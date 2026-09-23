//! `pns recap agent --stdin` and `pns recap git`: the two verbs an agent runs.
//!
//! THE ROUTE AND THE REPOSITORY ARE THE TWO THINGS NO UNIT TEST REACHES. The
//! domain tests pin the layout against facts handed to them; these pin the
//! facts and the wire.

mod support;

use std::process::Command;
use support::{Capture, Sandbox, plugin_command, run, stderr, stdout};

/// An arbitrary recap window for the two drills below, SINCE < UNTIL.
const SINCE: &str = "1756499000";
const UNTIL: &str = "1756500000";

/// AN AGENT'S OWN RECAP ON THE SAME WIRE, and the one route it has.
///
/// THE ROUTE IS THE WHOLE POINT OF THE COMMAND. The operator asked for these
/// recaps in a channel, and a recap that signed for one route and posted to
/// another would land nowhere: the gateway verifies the signature per route.
/// The gateway is PROXIED rather than moved, for the reason the night recap's
/// own route test states: `PNS_HERMES_URL` outranks the route name, so an
/// endpoint override cannot observe the path.
///
/// AND IT IS REFUSED HERE, 404, because that is the failure a route change
/// makes: the recap still exits 0 and posts nowhere else, so the refusal is
/// reported rather than retried.
#[test]
fn an_agent_recap_posts_once_on_the_default_route_and_exits_zero_when_refused() {
    let sandbox = Sandbox::new("recap-agent-route");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"gate-signing-key\" }\n",
    );
    let capture = Capture::builder(&sandbox, "recap-agent-route")
        .status(404)
        .start();

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
        ["POST /webhooks/pns-events HTTP/1.1"],
        "the recap took a route of its own: {raw}"
    );
    let body = raw.split("\r\n\r\n").last().expect("a posted body");
    assert!(
        body.contains("User Tasks"),
        "the posted body is not the one that was piped in: {body}"
    );
}

#[test]
fn an_agent_recap_posts_on_the_log_transport_the_config_selects() {
    let sandbox = Sandbox::new("recap-agent-discord");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"discord\"\nbot_token = \"token\"\n\
         [plugins.log.channels]\ndefault = \"1\"\npriority = \"2\"\n",
    );
    for channel in ["hermes", "discord"] {
        sandbox.stub_channel(
            channel,
            &format!("cat >>\"{}/{channel}.events\"", sandbox.display()),
        );
    }
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    let output = piped(
        command.args(["recap", "agent", "--stdin"]),
        b"Recap\n==========\n\n**User Tasks**\n1. `chezmoi apply`\n",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let posted = std::fs::read_to_string(sandbox.path("discord.events")).unwrap_or_default();
    assert!(
        posted.contains("User Tasks"),
        "discord was not handed the recap"
    );
    assert!(
        !sandbox.path("hermes.events").exists(),
        "the recap went to a transport the config did not select"
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

// --- the route a recap takes, on the wire ----------------------------------

#[test]
fn a_recap_the_gateway_refused_says_so_out_loud_and_still_exits_zero() {
    // THE HAND-RUN DRILL IS THE WHOLE REASON THIS MODE PRINTS. An operator who
    // has just prepared the gateway runs exactly this by hand to check it, and
    // MEASURED against an endpoint nothing is listening on, the mode
    // printed nothing and exited 0: indistinguishable from a recap that
    // arrived. `ReportMode::ReportOutcome` was already on the leg; nothing was
    // reading what it returned.
    //
    // AND EXIT 0 STILL, because that contract is the binary's and is not this
    // mode's to break: what is being fixed is silence, never the code. A typo
    // in the ARGUMENTS is the one thing that earns a 2, and its own test owns
    // that.
    let sandbox = Sandbox::new("recap-refused");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"gate-signing-key\" }\n",
    );

    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_STATE_DIR", sandbox.path("state"))
        // PORT 1 REFUSES IMMEDIATELY rather than hanging, so the failure this
        // test is about is the one it measures and not a deadline.
        .env("PNS_HERMES_URL", "http://127.0.0.1:1/webhooks/pns-events");
    sandbox.stub_notifier(&mut command);
    let output = run(command.args([
        "recap",
        "--since-epoch",
        SINCE,
        "--until-epoch",
        UNTIL,
        "--to",
        "durable",
    ]));

    let printed = stdout(&output);
    let said: Vec<&str> = printed
        .lines()
        .filter(|line| line.starts_with("pns: ") && line.contains("FAILED"))
        .collect();
    assert!(
        !said.is_empty(),
        "the recap mode said nothing about a post that never landed: {printed}"
    );
    assert!(
        said.iter().all(|line| line.contains("hermes gateway")),
        "the line does not name what refused it: {said:?}"
    );
}

/// THE ROUTE A RECAP TAKES, ON THE WIRE, and there is only the one.
///
/// THE ONE ASSERTION NO STUB CHANNEL CAN MAKE, for the reason the stale
/// alert's own route test states: `PNS_CHANNELS_DIR` leaves the native hermes
/// channel computing a URL nothing sends, and `PNS_HERMES_URL` outranks the
/// route, so an endpoint override cannot observe it either. So the gateway is
/// PROXIED rather than moved, exactly as that test does it, and the capture
/// answers 404 the way hermes answers for a route nobody prepared.
///
/// ONE REQUEST IS THE WHOLE DELIVERY. The recap used to try `pns-recap` and
/// fall back here; that route retired with its channel on 2026-09-15, so a
/// refusal is reported and nothing is posted twice.
#[test]
fn a_recap_posts_once_on_the_default_route_even_when_the_gateway_refuses_it() {
    let sandbox = Sandbox::new("recap-route");
    sandbox.write_config(
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"gate-signing-key\" }\n",
    );
    let capture = Capture::builder(&sandbox, "recap").status(404).start();

    let mut command = plugin_command(&sandbox);
    command
        .env("PNS_STATE_DIR", sandbox.path("state"))
        .env("HTTP_PROXY", capture.url())
        .env("http_proxy", capture.url());
    sandbox.stub_notifier(&mut command);
    // RUN BY HAND, which is the mode's other caller and the one a test can
    // wait for: the event path spawns this same mode detached, and the window
    // it would pass is exactly these two bounds.
    run(command.args([
        "recap",
        "--since-epoch",
        SINCE,
        "--until-epoch",
        UNTIL,
        "--to",
        "durable",
    ]));

    let raw = capture.finish();
    let posted: Vec<&str> = raw
        .lines()
        .filter(|line| line.starts_with("POST /webhooks/"))
        .collect();
    assert_eq!(
        posted,
        ["POST /webhooks/pns-events HTTP/1.1"],
        "the recap took a route of its own: {raw}"
    );
    let body = raw.split("\r\n\r\n").last().expect("a posted body");
    assert!(
        body.contains("While you were away"),
        "the posted body is not the composed recap: {body}"
    );
}
