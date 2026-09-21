use std::time::{SystemTime, UNIX_EPOCH};
/// Where this binary keeps what it has to remember between runs.
///
/// THE CONFIG FILE IS READ HERE rather than threaded through the fifty-odd
/// callers of this function: the directory is settled before anything the
/// caller holds exists, and `phone_marker_path` already reads the same file
/// for the same reason.
///
/// CACHED PER (HOME, PNS_STATE_DIR) PAIR, because the file costs milliseconds
/// to parse and this is read several times on a hook path a harness is
/// blocked on. Keyed rather than latched once: an in-process test suite that
/// changes either variable between cases must not keep reading the first
/// case's directory.
pub fn state_dir() -> std::path::PathBuf {
    static RESOLVED: std::sync::Mutex<Option<(String, Option<String>, std::path::PathBuf)>> =
        std::sync::Mutex::new(None);
    let home = std::env::var("HOME").unwrap_or_default();
    let state_dir_var = std::env::var("PNS_STATE_DIR").ok();
    let mut cached = RESOLVED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((cached_home, cached_var, path)) = cached.as_ref()
        && *cached_home == home
        && *cached_var == state_dir_var
    {
        return path.clone();
    }
    let resolved = std::path::PathBuf::from(
        crate::install_settings(&home)
            .state_dir
            .unwrap_or_else(|| format!("{home}/.local/state/pns")),
    );
    *cached = Some((home, state_dir_var, resolved.clone()));
    resolved
}
pub fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs())
}
