use std::time::{SystemTime, UNIX_EPOCH};
/// Where this binary keeps what it has to remember between runs.
pub fn state_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(
        std::env::var("PNS_STATE_DIR")
            .ok()
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| format!("{home}/.local/state/pns")),
    )
}
pub fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs())
}
