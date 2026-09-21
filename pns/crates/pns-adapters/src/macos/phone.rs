use super::proc_table::{ProcessTable, plain_device_name};
use crate::process::{PROBE_DEADLINE, bounded_call};
use pns_application::CommandRunner;
use std::sync::Arc;
use std::time::Duration;

pub const PGREP_PATH: &str = "/usr/bin/pgrep";

/// Where a terminal name resolves to. The name is validated before it is
/// joined, in `proc_table::terminal_name`, because this is the one place a
/// reading becomes a PATH.
pub(crate) const TTY_DIR: &str = "/dev";

/// The process ids `pgrep` printed, one per line, discarding anything that is
/// not a plain decimal id.
///
/// The RAW line is validated, never a trimmed copy of it, because the bash
/// reference matches its regex against the raw line too: padding is output this
/// module did not expect, and trimming it away would promote garbled output
/// into a trusted process id.
pub fn parse_pids(pgrep_output: &str) -> Vec<i32> {
    pgrep_output
        .lines()
        .filter(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|line| line.parse().ok())
        .collect()
}

/// The phone chain's own body, free of `&self` for the same reason as
/// `idle_reading`: `start` runs it on a spawned thread.
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
/// ONE SPAWN AND ONE WALK: `mosh-server` runs detached with no controlling
/// terminal of its own, so the terminal belongs to the client it forked, and
/// the process table answers the children and their terminals together. The
/// server selection stays shelled because macOS `pgrep -x` matches a name
/// field the native record does not reproduce exactly, and a phone selected
/// differently is a different reading.
///
/// FRESHEST WINS across every session found, and any step coming back
/// empty leaves None. None is never fresh, which drops the phone out of
/// the arbitration rather than parking the operator on it: a phone that
/// cannot be read must not silence the banner.
pub(crate) fn phone_reading<R: CommandRunner>(
    runner: &R,
    table: &Arc<dyn ProcessTable>,
    tty_dir: &str,
) -> Option<u64> {
    phone_reading_within(runner, table, tty_dir, PROBE_DEADLINE)
}

/// `phone_reading` with the deadline named, which is what lets a test drive a
/// process table that never answers without waiting the production window
/// out.
pub(crate) fn phone_reading_within<R: CommandRunner>(
    runner: &R,
    table: &Arc<dyn ProcessTable>,
    tty_dir: &str,
    deadline: Duration,
) -> Option<u64> {
    let servers = parse_pids(&runner.run(PGREP_PATH, &["-x", "mosh-server"])?);
    if servers.is_empty() {
        return None;
    }
    let table = Arc::clone(table);
    let terminals = bounded_call(deadline, move || table.child_terminals(&servers))?;
    newest_terminal_atime(tty_dir, &terminals)
}

/// The most recent access time among the terminals named, or None when not
/// one of them could be read.
///
/// The directory is a parameter so the lookup can be pointed at fixtures; in
/// production it is always `/dev`.
///
/// FILTERED AGAIN HERE, at the join itself, rather than trusted from the
/// `ProcessTable` that named them: `LibprocTable::terminal_name` already
/// refuses anything but a plain device name, but that guard lives one layer
/// away from where the path is actually built, and a second `ProcessTable`
/// impl could bypass it without touching this function.
pub fn newest_terminal_atime(tty_dir: &str, names: &[String]) -> Option<u64> {
    names
        .iter()
        .filter_map(|name| plain_device_name(name))
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
