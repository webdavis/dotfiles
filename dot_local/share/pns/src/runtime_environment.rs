use crate::*;

/// Where this binary keeps what it has to remember between runs.
pub(crate) fn state_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    resolve_path(
        std::env::var("PNS_STATE_DIR").ok().as_deref(),
        &format!("{home}/.local/state/pns"),
    )
}
pub(crate) fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs())
}
/// A deadline override in milliseconds, for tests that must prove expiry
/// without waiting out the production window.
pub(crate) fn env_deadline(variable: &str) -> Option<Duration> {
    std::env::var(variable)
        .ok()?
        .parse()
        .ok()
        .map(Duration::from_millis)
}
/// Every override the engine reads, out of the process environment.
pub(crate) fn overrides_from_env() -> Overrides {
    Overrides::from_env(
        &std::env::vars_os()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
}
/// A path from the environment, defaulting like bash's `${VAR:-default}`:
/// EMPTY means the default as much as unset does, because joining a filename
/// to an empty path resolves into the current directory and quietly delivers
/// nothing.
pub(crate) fn resolve_path(candidate: Option<&str>, default: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(
        candidate
            .filter(|value| !value.is_empty())
            .unwrap_or(default),
    )
}
/// The first executable of that name on PATH, absolute, or None. The click
/// string bakes it in because the click runs in a bare launchd context whose
/// PATH cannot find `~/.local/bin`.
pub(crate) fn executable_in_path(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "runtime_environment/tests.rs"]
mod runtime_environment_tests;
