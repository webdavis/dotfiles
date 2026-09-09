/// Whether the process a claim is named for has exited.
///
/// ONE ANSWER FOR EVERY CLAIM IN THIS DIRECTORY. The journal's holds and the
/// marker's claims both carry the id of the run that took them, and two copies
/// of this test would drift the day one of them learns something.
///
/// A LIVE PROCESS IS THE ONLY THING THAT DEFERS A CLAIM. `kill(pid, 0)`
/// answers `EPERM` for a process this user may not signal, which is still a
/// process that exists, so only `ESRCH` counts as gone. A pid the machine has
/// reused reads as alive, and what that costs is a batch that waits for the
/// first return after the process wearing its number exits, which is the same
/// shape of price `claim_by_rename` names for its own pid guard: a replay
/// deferred, never a replay destroyed and never one delivered twice.
pub fn owner_is_gone(owner: &str) -> bool {
    // THE PID IS THE SEGMENT BEFORE THE FIRST DOT (held.<pid>.<seq>); a bare
    // held.<pid> from an older build, and the marker's claim.<pid>, both parse
    // the same way.
    let owner = owner.split('.').next().unwrap_or_default();
    let Ok(pid) = owner.parse::<libc::pid_t>() else {
        return false;
    };
    // kill() reads non-positive values as the GROUP and BROADCAST forms, so a
    // hand-planted negative name must never reach it looking like a pid.
    if pid <= 0 {
        return false;
    }
    // SAFETY: `kill` with signal 0 sends nothing and only reports whether the
    // process exists.
    if unsafe { libc::kill(pid, 0) } != -1 {
        return false;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}
