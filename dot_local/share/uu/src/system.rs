//! The system's own answers: this clock, this host's name, and where a command
//! resolves on this PATH.
//!
//! THE LIBC AND ENVIRONMENT EDGE, kept out of every module that decides
//! something. Each function here is a question only the running machine can
//! answer, and each states what it does when the machine will not answer it.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// This instant in epoch seconds, or `None` for a clock set before 1970. The
/// caller decides what an unreadable clock means: the header prints it, the
/// marker refuses to move on it.
pub fn now_epoch() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs() as i64)
}

/// ISO 8601 UTC, computed from the same epoch the gap is, so the two figures
/// in one entry can never be sampled at different instants.
pub fn iso(epoch: i64) -> String {
    let mut when = std::mem::MaybeUninit::<libc::tm>::uninit();
    let seconds = epoch as libc::time_t;
    // SAFETY: gmtime_r writes into the caller's own tm and returns null only
    // when it wrote nothing, which is the branch below.
    let filled = unsafe { libc::gmtime_r(&seconds, when.as_mut_ptr()) };
    if filled.is_null() {
        return format!("epoch {epoch}");
    }
    let when = unsafe { when.assume_init() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        when.tm_year + 1900,
        when.tm_mon + 1,
        when.tm_mday,
        when.tm_hour,
        when.tm_min,
        when.tm_sec
    )
}

/// The machine this record is about. The channel aggregates unattended jobs
/// from more than one host, so an entry that does not name its host is not
/// investigable.
pub fn host() -> String {
    let mut name = [0_i8; 256];
    // SAFETY: the buffer and its length are this frame's own, and the result
    // is only read after a success, truncated at the first NUL.
    let read = unsafe { libc::gethostname(name.as_mut_ptr(), name.len()) };
    if read != 0 {
        return "unknown-host".to_string();
    }
    let bytes: Vec<u8> = name
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect();
    let full = String::from_utf8_lossy(&bytes).to_string();
    // The short name, matching `hostname -s`: a bonjour suffix says nothing
    // an entry needs.
    let short = full.split('.').next().unwrap_or_default().to_string();
    if short.is_empty() {
        "unknown-host".to_string()
    } else {
        short
    }
}

/// The operator's home, or `None` when the environment names none. Every mode
/// refuses outright without it, because the config, the state and the
/// rendered job all hang off it.
pub fn home() -> Option<String> {
    std::env::var("HOME").ok().filter(|home| !home.is_empty())
}

/// Where a command name resolves on this PATH, or `None`. An absolute or
/// relative path is answered from the filesystem directly, the way a shell
/// does.
pub fn resolve(command: &str) -> Option<PathBuf> {
    let runnable = |path: &Path| path.is_file();
    if command.contains('/') {
        let path = PathBuf::from(command);
        return runnable(&path).then_some(path);
    }
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| Path::new(entry).join(command))
        .find(|candidate| runnable(candidate))
}
