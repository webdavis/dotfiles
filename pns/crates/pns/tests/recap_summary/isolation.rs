//! The harness a recap summary runs: never the operator's own home, hooks or
//! tools, because a summary run that fired pns's own Stop hook would raise a
//! notification about itself.

use super::*;

/// `pns recap today --summarize` with `[recap.summarizer]` written as `table`
/// and a recording stub named `backend` first on PATH: the page it printed and
/// the sandbox the stub recorded into.
fn summarized_by(name: &str, backend: &str, table: &str) -> (String, Sandbox) {
    let sandbox = Sandbox::new(name);
    sandbox.allow_slow("the engine and its summarizer are two spawns");
    std::fs::create_dir_all(sandbox.state()).expect("the state dir");
    sandbox.write_config(&format!(
        "[recap]\nminimum_events = 1\n[recap.summarizer]\n{table}deadline = \"10s\"\n"
    ));
    planted(&sandbox, 60);
    let mut command = sandbox.pns_stateful();
    command.args(["recap", "today", "--summarize"]);
    sandbox.stub_recording_backend(&mut command, backend, ANSWER);
    let page = stdout(&run(&mut command));
    assert!(page.contains(ANSWER), "{page}");
    (page, sandbox)
}

/// What every recording stub answers.
const ANSWER: &str = "one blocked session is waiting on you";

#[test]
fn a_codex_summary_runs_ephemeral_and_read_only_in_the_stripped_home() {
    let (page, sandbox) = summarized_by(
        "recap-summary-codex-isolated",
        "codex",
        "type = \"codex\"\nmodel = \"gpt-6-luna\"\n",
    );
    let seen = sandbox.recorded_spawn(&page);
    seen.assert_codex_isolated(&sandbox.path(".config/pns/codex-home").display().to_string());
    assert_eq!(seen.after("-m"), Some("gpt-6-luna"), "{:?}", seen.argv);
}

#[test]
fn a_claude_summary_runs_in_safe_mode_with_no_tools() {
    let (page, sandbox) = summarized_by(
        "recap-summary-claude-isolated",
        "claude",
        "type = \"claude\"\nmodel = \"haiku\"\n",
    );
    let seen = sandbox.recorded_spawn(&page);
    assert!(
        seen.argv.contains(&"--safe-mode".to_string()),
        "{:?}",
        seen.argv
    );
    assert_eq!(seen.after("--tools"), Some(""), "{:?}", seen.argv);
    assert_eq!(seen.after("--model"), Some("haiku"), "{:?}", seen.argv);
}

#[test]
fn the_effort_reaches_each_backend_through_its_own_flag() {
    let (page, codex) = summarized_by(
        "recap-summary-codex-effort",
        "codex",
        "type = \"codex\"\neffort = \"low\"\n",
    );
    let seen = codex.recorded_spawn(&page);
    assert!(
        seen.passed("-c", "model_reasoning_effort=\"low\""),
        "{:?}",
        seen.argv
    );
    let (page, claude) = summarized_by(
        "recap-summary-claude-effort",
        "claude",
        "type = \"claude\"\neffort = \"low\"\n",
    );
    let seen = claude.recorded_spawn(&page);
    assert_eq!(seen.after("--effort"), Some("low"), "{:?}", seen.argv);
}
