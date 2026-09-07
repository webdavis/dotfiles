use pns_application::CommandRunner;
pub const PGREP_PATH: &str = "/usr/bin/pgrep";
pub const PS_PATH: &str = "/bin/ps";

/// Where a terminal name from `ps` resolves to. The name is validated before
/// it is joined, because this is the one place a reading becomes a PATH.
pub(crate) const TTY_DIR: &str = "/dev";

/// The process ids `pgrep` printed, one per line, discarding anything that is
/// not a plain decimal id.
///
/// The RAW line is validated, never a trimmed copy of it, because the bash
/// reference matches its regex against the raw line too: padding is output this
/// module did not expect, and trimming it away would promote garbled output
/// into a trusted process id.
pub fn parse_pids(pgrep_output: &str) -> Vec<String> {
    pgrep_output
        .lines()
        .filter(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// The terminal names `ps` printed, one per line, discarding every line that
/// is not one.
///
/// THIS IS A TRUST BOUNDARY, because the name is about to become a path under
/// `/dev`. A process with no controlling terminal prints `??`, and that is
/// only the benign case: anything not plain alphanumeric is refused outright,
/// so no reading can carry a slash or a `..` into the join below.
///
/// The name is trimmed first, unlike a process id: `ps -o tty=` pads its
/// column to a fixed width, so the padding is the format rather than the
/// garbled output that padding around a pid would be.
pub fn parse_tty_names(ps_output: &str) -> Vec<&str> {
    ps_output
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect()
}

/// The phone chain's own body, free of `&self` for the same reason as
/// `idle_reading` above: `start` runs it on a spawned thread.
///
/// WHEN THE PHONE LAST TYPED, SCROLLED OR TAPPED INTO THE SESSION, as the
/// access time of the pty its mosh client is attached to.
///
/// THE READING IS ATIME, AND THAT IS THE WHOLE TRICK. On macOS a tty's
/// atime moves when something is written INTO it and its mtime moves when
/// something is read OUT of it, so atime is input and mtime is the agent
/// talking back. Proven live on 2026-08-15 in both directions: a scroll on
/// the phone moved the mosh pty's atime while typing at the desk left it
/// untouched. That is what makes this comparable with the desk's own idle
/// clock instead of the byte sample it replaces, which passive viewing
/// could not move at all.
///
/// THREE BOUNDED SPAWNS, never one per process: `mosh-server` runs
/// detached with no controlling terminal of its own, so the terminal
/// belongs to the client it forked, and both `pgrep -P` and `ps -p` take
/// the whole list of ids at once.
///
/// FRESHEST WINS across every session found, and any step coming back
/// empty leaves None. None is never fresh, which drops the phone out of
/// the arbitration rather than parking the operator on it: a phone that
/// cannot be read must not silence the banner.
pub(crate) fn phone_reading<R: CommandRunner>(runner: &R, tty_dir: &str) -> Option<u64> {
    let servers = parse_pids(&runner.run(PGREP_PATH, &["-x", "mosh-server"])?);
    let clients = parse_pids(&pgrep_children(runner, &servers)?);
    if clients.is_empty() {
        return None;
    }
    let terminals = runner.run(PS_PATH, &["-o", "tty=", "-p", &clients.join(",")])?;
    newest_terminal_atime(tty_dir, &terminals)
}

/// Every child of the given parents, in one call. No parents means no call:
/// `pgrep -P` with an empty list is a usage error, not a query answering
/// "none".
fn pgrep_children<R: CommandRunner>(runner: &R, parents: &[String]) -> Option<String> {
    if parents.is_empty() {
        return None;
    }
    runner.run(PGREP_PATH, &["-P", &parents.join(",")])
}

/// The most recent access time among the terminals `ps` named, or None when
/// not one of them could be read.
///
/// The directory is a parameter so the lookup can be pointed at fixtures; in
/// production it is always `/dev`.
pub fn newest_terminal_atime(tty_dir: &str, ps_output: &str) -> Option<u64> {
    parse_tty_names(ps_output)
        .into_iter()
        .filter_map(|name| atime_secs(&format!("{tty_dir}/{name}")))
        .max()
}

/// A file's access time in whole seconds since the epoch, or None when it
/// cannot be read. A plain `stat`, which does not itself count as an access,
/// so taking the reading never disturbs it.
fn atime_secs(path: &str) -> Option<u64> {
    Some(
        std::fs::metadata(path)
            .ok()?
            .accessed()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs(),
    )
}
