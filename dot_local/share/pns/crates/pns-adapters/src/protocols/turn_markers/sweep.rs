use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

// A Stop may never arrive. Seven days preserves long sessions while bounding
// abandoned starts; exact-boundary, future and unreadable clocks stay intact.
const TURN_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;

pub fn sweep_turn_markers(state: &Path, now: Option<u64>) {
    let Some(now) = now else {
        return;
    };
    for entry in std::fs::read_dir(state).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let Some(session) = name
            .to_str()
            .and_then(|name| name.strip_prefix("session-"))
            .and_then(|name| name.strip_suffix(".start"))
        else {
            continue;
        };
        if !pns_domain::safety::session_id_is_safe(session) {
            continue;
        }
        let path = entry.path();
        if expired(&path, now) {
            claim_expired(&path, now);
        }
    }
}

fn expired(path: &Path, now: u64) -> bool {
    crate::readable_state_file(path, crate::RING_READ_MAX)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .and_then(|at| now.checked_sub(at))
        .is_some_and(|age| age > TURN_RETENTION_SECS)
}

fn claim_expired(path: &Path, now: u64) {
    let claim = path.with_extension(format!("start.sweep.{}", std::process::id()));
    // Never replace an abandoned claim from a reused pid or a concurrent
    // invocation in this process. Only this successful creator may rename here.
    let Ok(_reservation) = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(crate::STATE_FILE_MODE)
        .open(&claim)
    else {
        return;
    };
    if std::fs::rename(path, &claim).is_err() {
        let _ = std::fs::remove_file(&claim);
        return;
    }
    finish_claim(path, &claim, now);
}

fn finish_claim(path: &Path, claim: &Path, now: u64) {
    // A later prompt can publish at the ordinary name after the claim. Read
    // only the owned file again and never overwrite that later arrival.
    if expired(claim, now) || std::fs::hard_link(claim, path).is_ok() {
        let _ = std::fs::remove_file(claim);
    }
    // If restoration is refused, retain the claim for manual recovery. A
    // fresh or unreadable start must not disappear to make housekeeping tidy.
}

#[cfg(test)]
mod tests;
