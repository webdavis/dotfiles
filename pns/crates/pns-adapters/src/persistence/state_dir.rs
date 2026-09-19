use std::time::{SystemTime, UNIX_EPOCH};
/// Where this binary keeps what it has to remember between runs.
///
/// THE CONFIG FILE IS READ HERE rather than threaded through the fifty-odd
/// callers of this function: the directory is settled before anything the
/// caller holds exists, and `phone_marker_path` already reads the same file
/// for the same reason.
///
/// ONCE PER PROCESS, because the file costs milliseconds to parse and this is
/// read several times on a hook path a harness is blocked on. A process that
/// outlives an edit keeps the directory it started on, which is what a daemon
/// holding open handles under it needs anyway.
pub fn state_dir() -> std::path::PathBuf {
    static RESOLVED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    RESOLVED
        .get_or_init(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            std::path::PathBuf::from(
                crate::install_settings(&home)
                    .state_dir
                    .unwrap_or_else(|| format!("{home}/.local/state/pns")),
            )
        })
        .clone()
}
pub fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs())
}
