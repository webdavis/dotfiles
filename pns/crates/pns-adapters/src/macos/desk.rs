use pns_application::CommandRunner;

/// The idle counter's own units, as the registry reports them.
const IOREG_IDLE_KEY: &str = "HIDIdleTime";

/// The console lock aggregate's key, MATCHED QUOTED so the key's own token is
/// what the search anchors on rather than any name that merely ends in it.
const IOREG_LOCK_KEY: &str = "\"IOConsoleLocked\"";

/// Absolute, because a probe must not resolve a system binary through a PATH
/// it does not control.
pub const IOREG_PATH: &str = "/usr/sbin/ioreg";

/// The idle nanosecond count, taken from the FIRST line that carries the key.
///
/// The registry prints one key per line as `"Key" = value`; the count is the
/// last whitespace-separated field. A line without one, or no line at all,
/// yields None, which every consumer already reads as "unknown".
///
/// Contaminated output is refused WHOLESALE rather than searched, matching the
/// bash reference's grep, which treats NUL-bearing input as binary. A
/// replacement character means the runner already substituted an invalid byte,
/// the same corruption. Trusting either can coerce to 0, which reads as
/// "actively typing" and silently drops the push.
pub fn parse_idle_nanoseconds(ioreg_output: &str) -> Option<&str> {
    if ioreg_output.contains(['\0', '\u{FFFD}']) {
        return None;
    }
    ioreg_output
        .lines()
        .find(|line| line.contains(IOREG_IDLE_KEY))
        .and_then(|line| line.split_whitespace().last())
}

/// Whether the console is locked, from the Root dictionary's own aggregate.
///
/// `Some(true)` for `Yes`, `Some(false)` for `No`, `None` for a key that is
/// absent or carries anything else. THE FAIL DIRECTION IS DELIBERATE and the
/// decision states it (`surface::surface`): only `Some(true)` locks, so a
/// reading nobody could take leaves the shipped desk-freshness behavior in
/// place instead of killing the desk banner permanently wherever this key is
/// renamed or dropped.
///
/// WHAT THE QUOTED KEY KEEPS OUT is the `"IOConsoleUsers"` line printed
/// beside it: that array holds one dictionary per login session, each with a
/// nested `CGSSessionScreenIsLocked` of its own, and reading a per-session
/// flag would mean picking the console session first. `IOConsoleLocked` is
/// the aggregate the kernel already computed.
pub fn parse_screen_locked(ioreg_output: &str) -> Option<bool> {
    match ioreg_output
        .lines()
        .find(|line| line.contains(IOREG_LOCK_KEY))?
        .split_whitespace()
        .last()?
    {
        "Yes" => Some(true),
        "No" => Some(false),
        _ => None,
    }
}

/// The idle probe's own body, free of `&self` so `start` can run it on a
/// spawned thread against a cloned `Arc<R>` rather than borrowing the struct
/// across threads. The trait impl below runs the SAME function inline.
pub(crate) fn idle_reading<R: CommandRunner>(runner: &R) -> Option<u64> {
    let ioreg_output = runner.run(IOREG_PATH, &["-c", "IOHIDSystem"])?;
    pns_domain::idle_secs_from_ns(parse_idle_nanoseconds(&ioreg_output)?)
}

/// The lock probe's own body, same reason as `idle_reading` beside it.
///
/// The Root node with its own properties, which is the one node carrying
/// the console aggregate; `-d1` stops the walk there rather than printing
/// the tree under it.
///
/// A SECOND `ioreg` SPAWN, not a second parse of the idle probe's output:
/// that one asks for the `IOHIDSystem` class and the aggregate is not in
/// it. This read is the cheaper of the two by a wide margin (92KB against
/// 294KB, measured on dresden 2026-08-28) and only happens where the idle
/// reading it exists to qualify was taken.
pub(crate) fn lock_reading<R: CommandRunner>(runner: &R) -> Option<bool> {
    parse_screen_locked(&runner.run(IOREG_PATH, &["-n", "Root", "-d1"])?)
}
