use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The turn's start marker, so the Stop hook can measure the turn that just
/// finished rather than the whole session.
pub fn start_of_turn(state: &Path, session_id: &str, now: Option<u64>) {
    let Some(marker) = turn_marker(state, session_id) else {
        return;
    };
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Only when none is there: a second prompt inside one turn must not
    // restart the clock.
    // NO CLOCK IS NO MARKER, never a marker at epoch zero: the same rule
    // `update_blocked_marker` states beside its own clock. A marker at zero
    // would measure the turn from 1970, so `consume_turn_marker` would call a
    // two-second turn long-running and it would earn the watch card and the
    // pulse; no marker measures nothing, and `session_was_long` reads that as
    // not long.
    if let Some(now) = now
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(crate::STATE_FILE_MODE)
            .open(&marker)
    {
        let _ = file.write_all(now.to_string().as_bytes());
    }
}
/// The turn's marker path, or None for a session id that cannot become a
/// filename. The id arrives in the harness payload, and `..` in it would
/// escape the state directory.
fn turn_marker(state: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    if !pns_domain::safety::session_id_is_safe(session_id) {
        return None;
    }
    Some(state.join(format!("session-{session_id}.start")))
}
/// How long the finished turn ran, CLAIMING the marker first.
///
/// The claim is a rename, which is atomic: two Stops racing the same turn
/// cannot both read it and both pulse, because only one rename can succeed.
/// Reading first and unlinking after left that window open, and an unlink
/// that failed left the marker wedged for every later turn.
///
/// It runs BEFORE the reply and the condenser for the same reason. Stop is
/// asynchronous, so the next prompt can arrive while this one is still
/// condensing: with the marker still on disk that prompt writes nothing, and
/// this Stop then deletes the marker its successor was relying on. Claiming
/// up front also keeps the condenser's own latency out of the elapsed time it
/// is measuring.
///
/// The value is VALIDATED before it reaches arithmetic: a truncated write or
/// a hand edit must be a decision, not a crash.
pub fn consume_turn_marker(
    state: &Path,
    session_id: &str,
    now: impl FnOnce() -> Option<u64>,
) -> Option<u64> {
    let marker = turn_marker(state, session_id)?;
    let claim = marker.with_extension(format!("claim.{}", std::process::id()));
    std::fs::rename(&marker, &claim).ok()?;
    let started = std::fs::read_to_string(&claim);
    let _ = std::fs::remove_file(&claim);
    let started: u64 = started.ok()?.trim().parse().ok()?;
    Some(now()?.saturating_sub(started))
}

mod sweep;
pub use sweep::sweep_turn_markers;

#[cfg(test)]
mod tests;
