//! The IO edge: the five probe traits implemented against the real machine.
//!
//! WHAT LIVES HERE AND WHAT DOES NOT. Everything here runs a command and hands
//! the bytes to a parser; every parser is a free function taking `&str`, so a
//! test drives fixture output and never spawns anything. The DECISIONS all live
//! in `surface`, `presence` and `routing`, which is why nothing in this module
//! compares, thresholds or judges: it says what the machine reported, and
//! `surface` says what that means.
//!
//! The runner seam exists for the same reason: a test substitutes the command
//! output, so the suite never reads the live machine. That matters more than
//! usual here, because these readings are of the developer's own desk and
//! phone, and a suite that took them would answer differently every run.

use crate::macos::desk::{idle_reading, lock_reading};
use crate::macos::phone::{TTY_DIR, phone_reading};
use crate::readable_state_file;
use pns_application::CommandRunner;
use std::sync::Arc;

/// What the desk thread hands back on join: the idle reading, and the lock
/// reading only where idle parsed. See `join_desk`.
type DeskHandle = std::thread::JoinHandle<(Option<u64>, Option<bool>)>;

/// The five probes: four read the machine through commands, and the marker
/// reads the filesystem directly, because an mtime needs no subprocess.
///
/// One struct rather than five, because they share the runner and a caller
/// composes the traits it needs. SOLID: the command probes depend on the
/// runner abstraction, never on `Command` directly, so that edge substitutes
/// in tests; the marker's substitution point is the path it is handed.
/// ONE PROBE SET IS ONE READING, however many consumers ask for it. Each
/// reading is taken at most once and remembered, including the reading that
/// came back empty, because an unreadable probe is an answer too.
///
/// The blocked path is what makes this load-bearing: it asks where the
/// operator is twice by design, once to decide whether an approval is
/// forwarded to the phone at all and again to decide what the notification
/// delivers. Taking the measurement twice lets a freshness boundary fall
/// between them, which cards a phone with no round trip behind it.
pub struct SystemProbes<R: CommandRunner> {
    runner: Arc<R>,
    marker_path: String,
    /// Where a phone reading's terminal name resolves to. Always `TTY_DIR`
    /// in production; a test points it at a fixture directory instead of
    /// stubbing `newest_terminal_atime` a second time, see `with_tty_dir`.
    tty_dir: String,
    idle: std::cell::OnceCell<Option<u64>>,
    marker_mtime: std::cell::OnceCell<Option<u64>>,
    phone_atime: std::cell::OnceCell<Option<u64>>,
    screen_locked: std::cell::OnceCell<Option<bool>>,
    now: std::cell::OnceCell<Option<u64>>,
    /// Where the presence reading is published, which is the daemon's
    /// state file. EMPTY UNTIL A CALLER POINTS THIS AT ONE, see
    /// `with_presence_path`: an unpointed probe set reads no line, which
    /// is the Unknown every consumer of this reading already fails to.
    presence_path: String,
    presence_line: std::cell::OnceCell<Option<String>>,
    /// Set by `start` and taken by the first read that needs it: see
    /// `ProbeStart` and `join_desk`. `None` means either nothing was ever
    /// started, or a thread already ran and was already joined.
    desk_handle: std::cell::Cell<Option<DeskHandle>>,
    /// The phone twin of `desk_handle`.
    phone_handle: std::cell::Cell<Option<std::thread::JoinHandle<Option<u64>>>>,
}

mod readings;
mod session;
mod start;

#[cfg(test)]
mod tests;
