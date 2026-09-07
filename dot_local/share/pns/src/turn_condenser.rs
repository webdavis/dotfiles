use crate::*;

/// The turn condensed to a state and a sentence, by a cheap model when one
/// answers and by trimming the reply when it does not.
pub(crate) fn condense(reply: &str) -> (String, String) {
    let fallback = || ("done".to_string(), pns::render::preview(reply));
    // The re-entry guard: the condenser is itself an agent run, and its own
    // Stop hook would call this again. The stripped home below installs no
    // hooks at all, which is the hard guarantee; this is the cheap one.
    if std::env::var("PNS_SUMMARIZING").is_ok() {
        return fallback();
    }
    let Some(home) = condenser_home() else {
        return fallback();
    };
    let codex = std::env::var("CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
    let mut command = Command::new(&codex);
    command
        .args(["exec", "--ephemeral", "--skip-git-repo-check", "-C"])
        .arg(&home)
        .args(["-s", "read-only", "-"])
        .env("PNS_SUMMARIZING", "1")
        .env("CODEX_HOME", &home);
    let deadline = env_deadline("PNS_CONDENSER_DEADLINE_MS").unwrap_or(CONDENSER_DEADLINE);
    match run_bounded(
        command,
        Some(&condenser_prompt(reply)),
        deadline,
        PROBE_READ_MAX,
    )
    .as_deref()
    .and_then(condenser_verdict)
    {
        Some((state, summary)) => (state, summary.trim().to_string()),
        None => fallback(),
    }
}
/// A private, stripped Codex home: a minimal config (fast model, low
/// reasoning) and the live auth symlinked, with NO hooks or plugins. That cuts
/// the load (~9s to ~3s) and means the condenser run has no Stop hook of its
/// own, which is the hard guarantee against a pns-to-codex-to-pns loop.
/// It is created owner-only, because it points at the live Codex credentials.
fn condenser_home() -> Option<std::path::PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let user_home = std::env::var("HOME").unwrap_or_default();
    let home = resolve_path(
        std::env::var("PNS_CODEX_HOME").ok().as_deref(),
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
/// The condenser is a model call on a notification path: worth a few seconds,
/// never worth holding a turn's report.
const CONDENSER_DEADLINE: Duration = Duration::from_secs(30);
