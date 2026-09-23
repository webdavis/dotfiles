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
    let codex = std::env::var("PNS_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
    let mut command = Command::new(&codex);
    command.args(["exec", "--skip-git-repo-check"]);
    if isolate(&mut command).is_none() {
        return fallback();
    }
    command.arg("-");
    // THE CONFIG FILE IS READ HERE rather than threaded through the hook
    // path, the way `state_dir` reads it for the same reason.
    let user_home = std::env::var("HOME").unwrap_or_default();
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
/// Point a `codex exec` command at the stripped home below: ephemeral, in a
/// read-only sandbox, with every tool a summary has no use for switched off,
/// and marked as a summarizer run. Both the turn summarizer and the recap's
/// `codex` kind run through this. None when the home cannot be made.
pub(crate) fn isolate(command: &mut Command) -> Option<()> {
    let user_home = std::env::var("HOME").unwrap_or_default();
    let home = summarizer_home(&user_home, std::env::var("PNS_CODEX_HOME").ok().as_deref())?;
    command
        .args(["--ephemeral", "-s", "read-only", "-C"])
        .arg(&home)
        .args(
            DISABLED_FEATURES
                .iter()
                .flat_map(|feature| ["--disable", feature]),
        )
        .args(["-c", "web_search=\"disabled\""])
        .env("PNS_SUMMARIZING", "1")
        .env("CODEX_HOME", &home);
    Some(())
}
/// The Codex features a summary run is started without. MEASURED on
/// codex-cli 0.156.0 against a local capture of the model request: with these
/// off and web search disabled, the only tool offered is `request_user_input`.
/// The stripped home also carries the account's remotely installed plugins and
/// their app connectors, which `apps` and `plugins` keep out.
const DISABLED_FEATURES: [&str; 8] = [
    "shell_tool",
    "unified_exec",
    "apps",
    "plugins",
    "hooks",
    "multi_agent",
    "goals",
    "view_image",
];
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
    link_auth(&home, &format!("{user_home}/.codex/auth.json"));
    Some(home)
}
/// Point the home's `auth.json` at the live Codex credentials.
///
/// THE HOME IS SHARED by the recap, the doctor and the Stop hook's summarizer,
/// which can run at once. A link that is already right is left alone, and a new
/// one is made under this process's own name and renamed into place, so a run
/// reading auth at that moment finds the old link or the new one.
fn link_auth(home: &std::path::Path, target: &str) {
    let auth = home.join("auth.json");
    if std::fs::read_link(&auth).is_ok_and(|current| current.as_os_str() == target) {
        return;
    }
    let staged = home.join(format!("auth.json.{}", std::process::id()));
    let _ = std::fs::remove_file(&staged);
    if std::os::unix::fs::symlink(target, &staged).is_ok()
        && std::fs::rename(&staged, &auth).is_err()
    {
        let _ = std::fs::remove_file(&staged);
    }
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
