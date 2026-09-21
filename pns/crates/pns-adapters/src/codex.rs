use crate::{PROBE_READ_MAX, resolve_path, run_bounded};
use pns_domain::{summarizer_prompt, summarizer_verdict};
use std::process::Command;
use std::time::Duration;

/// The turn summarized to a state and a sentence, by a cheap model when one
/// answers and by trimming the reply when it does not.
pub fn summarize(reply: &str) -> (String, String) {
    let fallback = || ("done".to_string(), pns_domain::render::preview(reply));
    // The re-entry guard: the summarizer is itself an agent run, and its own
    // Stop hook would call this again. The stripped home below installs no
    // hooks at all, which is the hard guarantee; this is the cheap one.
    if std::env::var("PNS_SUMMARIZING").is_ok() {
        return fallback();
    }
    let user_home = std::env::var("HOME").unwrap_or_default();
    let Some(home) = summarizer_home(&user_home, std::env::var("PNS_CODEX_HOME").ok().as_deref())
    else {
        return fallback();
    };
    let codex = std::env::var("PNS_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
    let mut command = Command::new(&codex);
    command
        .args(["exec", "--ephemeral", "--skip-git-repo-check", "-C"])
        .arg(&home)
        .args(["-s", "read-only", "-"])
        .env("PNS_SUMMARIZING", "1")
        .env("CODEX_HOME", &home);
    // THE CONFIG FILE IS READ HERE rather than threaded through the hook
    // path, the way `state_dir` reads it for the same reason.
    let deadline = turn_deadline(crate::install_settings(&user_home).summarizer_deadline);
    match run_bounded(
        command,
        Some(&summarizer_prompt(reply)),
        deadline,
        PROBE_READ_MAX,
    )
    .as_deref()
    .and_then(summarizer_verdict)
    {
        Some((state, summary)) => (state, summary.trim().to_string()),
        None => fallback(),
    }
}
/// A private, stripped Codex home: a minimal config (fast model, low
/// reasoning) and the live auth symlinked, with NO hooks or plugins. That cuts
/// the load (~9s to ~3s) and means the summarizer run has no Stop hook of its
/// own, which is the hard guarantee against a pns-to-codex-to-pns loop.
/// It is created owner-only, because it points at the live Codex credentials.
fn summarizer_home(user_home: &str, home_override: Option<&str>) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let home = resolve_path(
        home_override,
        &format!("{user_home}/.config/pns/codex-home"),
    );
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&home)
        .ok()?;
    let config = home.join("config.toml");
    if !config.exists() {
        let written = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&config)
            .map(|mut file| {
                std::io::Write::write_all(
                    &mut file,
                    b"model = \"gpt-5.5\"\nmodel_reasoning_effort = \"low\"\n",
                )
            });
        let _ = written;
    }
    let auth = home.join("auth.json");
    let _ = std::fs::remove_file(&auth);
    let _ = std::os::unix::fs::symlink(format!("{user_home}/.codex/auth.json"), &auth);
    Some(home)
}
/// The most of `[recap] summarizer_deadline` the TURN summarizer may take.
/// The key's own generous default is the budget for a whole recap episode in
/// a detached process nobody is waiting on, while this call sits inside a Stop
/// hook the harness is blocked on: a model call there is worth a few seconds
/// and never worth holding a turn's report.
const TURN_SUMMARIZER_CEILING: Duration = Duration::from_secs(30);

/// The configured bound, held to that ceiling.
fn turn_deadline(configured: Duration) -> Duration {
    configured.min(TURN_SUMMARIZER_CEILING)
}

#[cfg(test)]
mod tests;
