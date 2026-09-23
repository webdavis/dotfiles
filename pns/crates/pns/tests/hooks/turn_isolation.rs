use super::*;

/// The Stop hook's own summarizer is a Codex run, so in the operator's
/// `~/.codex` it would fire this same Stop hook about itself.
#[test]
fn the_turn_summarizer_runs_ephemeral_and_read_only_in_the_stripped_home() {
    let sandbox = Sandbox::new("hook-stop-summarizer-isolated");
    let home = sandbox.path("codex-home");
    let mut command = sandbox.pns();
    sandbox.stub_recording_backend(&mut command, "codex", "done|summarized");
    command
        .env("PNS_CODEX_BIN", sandbox.path("bin/codex"))
        .env("PNS_CODEX_HOME", &home);
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    let detail = sandbox.event("hermes")["detail"].to_string();
    assert!(detail.contains("summarized"), "the stub answered: {detail}");
    sandbox
        .recorded_spawn(&detail)
        .assert_codex_isolated(&home.display().to_string());
}
