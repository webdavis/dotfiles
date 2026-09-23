//! The harness a recap summary runs: never the operator's own home, hooks or
//! tools, because a summary run that fired pns's own Stop hook would raise a
//! notification about itself.

use super::*;

/// What a stub backend saw: its argument words and the two environment
/// values the isolation sets.
struct Seen {
    page: String,
    argv: Vec<String>,
    codex_home: String,
    summarizing: String,
}

impl Seen {
    /// The word right after `flag`, or None when `flag` was not passed.
    fn after(&self, flag: &str) -> Option<&str> {
        let at = self.argv.iter().position(|word| word == flag)?;
        self.argv.get(at + 1).map(String::as_str)
    }
}

/// `pns recap today --summarize` with `[recap.summarizer]` written as `table`
/// and a stub named `backend` first on PATH, recording what it was handed.
fn summarized_by(name: &str, backend: &str, table: &str) -> Seen {
    let sandbox = Sandbox::new(name);
    sandbox.allow_slow("the engine and its summarizer are two spawns");
    std::fs::create_dir_all(sandbox.state()).expect("the state dir");
    sandbox.write_config(&format!(
        "[recap]\nminimum_events = 1\n[recap.summarizer]\n{table}deadline = \"10s\"\n"
    ));
    planted(&sandbox, 60);
    let argv = sandbox.path("backend.argv");
    let env = sandbox.path("backend.env");
    let mut command = sandbox.pns_stateful();
    command.args(["recap", "today", "--summarize"]);
    sandbox.stub_on_path(
        &mut command,
        backend,
        &format!(
            "cat >/dev/null\n\
             for word in \"$@\"; do printf '%s\\n' \"$word\"; done >\"{}\"\n\
             printf '%s\\n%s\\n' \"${{CODEX_HOME-unset}}\" \"${{PNS_SUMMARIZING-unset}}\" >\"{}\"\n\
             printf 'one blocked session is waiting on you\\n'",
            argv.display(),
            env.display()
        ),
    );
    let page = stdout(&run(&mut command));
    let recorded = |path: &std::path::Path| -> Vec<String> {
        std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("the stub never ran ({error}): {page}"))
            .lines()
            .map(str::to_string)
            .collect()
    };
    let env = recorded(&env);
    Seen {
        argv: recorded(&argv),
        codex_home: env[0].clone(),
        summarizing: env[1].clone(),
        page,
    }
}

#[test]
fn a_codex_summary_runs_ephemeral_and_read_only_in_the_stripped_home() {
    let seen = summarized_by(
        "recap-summary-codex-isolated",
        "codex",
        "type = \"codex\"\nmodel = \"gpt-6-luna\"\n",
    );
    assert!(
        seen.page.contains("one blocked session is waiting on you"),
        "{}",
        seen.page
    );
    assert!(
        seen.argv.contains(&"--ephemeral".to_string()),
        "{:?}",
        seen.argv
    );
    assert_eq!(seen.after("-s"), Some("read-only"), "{:?}", seen.argv);
    assert_eq!(seen.after("-m"), Some("gpt-6-luna"), "{:?}", seen.argv);
    // THE STRIPPED HOME IS BOTH THE WORKING ROOT AND CODEX_HOME, and it is
    // pns's own directory rather than the operator's `~/.codex`.
    assert_eq!(
        seen.after("-C"),
        Some(seen.codex_home.as_str()),
        "{:?}",
        seen.argv
    );
    assert!(
        seen.codex_home.ends_with("/.config/pns/codex-home"),
        "{}",
        seen.codex_home
    );
    assert_eq!(seen.summarizing, "1", "the re-entry guard is set");
}

#[test]
fn a_claude_summary_runs_in_safe_mode_with_no_tools() {
    let seen = summarized_by(
        "recap-summary-claude-isolated",
        "claude",
        "type = \"claude\"\nmodel = \"haiku\"\n",
    );
    assert!(
        seen.page.contains("one blocked session is waiting on you"),
        "{}",
        seen.page
    );
    assert!(
        seen.argv.contains(&"--safe-mode".to_string()),
        "{:?}",
        seen.argv
    );
    assert_eq!(seen.after("--tools"), Some(""), "{:?}", seen.argv);
    assert_eq!(seen.after("--model"), Some("haiku"), "{:?}", seen.argv);
}
