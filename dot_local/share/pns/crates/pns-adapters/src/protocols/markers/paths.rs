use pns_domain::lights::WORKING_SWEEP;
/// Where a pane's loop lease lives: one file per pane holding one epoch.
///
/// A DIRECTORY, LIKE THE WAITS, because several panes can each be running a
/// loop and each must be the only writer and the only ordinary remover of its
/// own file. One shared file would be a lease any other pane erases.
pub fn lease_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-loop")
}

/// One pane's lease path, or None for a pane id that cannot become a filename.
pub fn lease_marker(state_dir: &std::path::Path, pane: &str) -> Option<std::path::PathBuf> {
    pns_domain::safety::pane_file_is_safe(pane).then(|| lease_dir(state_dir).join(pane))
}

/// One run's private name for a marker it has taken to remove.
pub fn sweep_claim(directory: &std::path::Path, name: &str, pid: u32) -> std::path::PathBuf {
    directory.join(format!("{name}{WORKING_SWEEP}{pid}"))
}

/// Where the needs markers live: one file per waiting session.
pub fn blocked_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-blocked")
}

/// One session's marker path, or None for a session id that cannot become a
/// filename.
///
/// THE SESSION ID AND NOT THE PANE, and the difference is a path escape.
/// `pane_is_safe` permits `..` because a pane id becomes a shell WORD, never a
/// filename; `session_id_is_safe` forbids it and already backs a filename in
/// this same directory (`session-<id>.start`). Reusing it writes no new
/// predicate and opens no new door.
pub fn blocked_marker(state_dir: &std::path::Path, session_id: &str) -> Option<std::path::PathBuf> {
    pns_domain::safety::session_id_is_safe(session_id)
        .then(|| blocked_dir(state_dir).join(session_id))
}
