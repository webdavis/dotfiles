use super::*;
/// The oldest epoch a LIVE shell is holding, with the markers whose shells are
/// gone REMOVED on the way through.
///
/// THE SWEEP LIVES WITH THE READ, for `sweep_blocked`' reason: the tick is the
/// only process that ever looks in this directory, and a shell killed
/// mid-command leaves a file its own precmd will never run to remove.
///
/// THE OLDEST AND NOT THE FRESHEST. Several panes hold markers at once, and
/// the reader's one question is how long work has been going: the freshest
/// would restart the breathe clock every time any pane ran anything, so a
/// build running for an hour beside a prompt somebody keeps typing at would
/// never reach a threshold measured in minutes.
///
/// AN EPOCH THAT CANNOT BE READ IS NOT SWEPT WHILE ITS SHELL IS ALIVE, which
/// is the one place this differs from `sweep_blocked`. The shell publishes with a
/// truncating redirect, so a tick landing between that open and the write sees
/// an empty file for a command that is genuinely starting; unlinking it there
/// wins the race and the build then runs to completion with no marker at all.
/// Nothing accumulates by leaving it: the pid in the name collects the file
/// when that shell ends.
pub fn sweep_shell_markers(state: &Path) -> Option<u64> {
    let mut oldest: Option<u64> = None;
    for entry in std::fs::read_dir(state.join(LIGHTS_SHELL_DIR))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // THE SAME LIVENESS ANSWER THE CLAIMS USE, so this binary has one
        // reading of "that process is gone" rather than two that can drift.
        // The positive-pid test comes first because `kill()` reads 0 as this
        // process's own group and -1 as every process the user owns, and
        // because a name that is not a pid at all is litter nothing else here
        // would ever age out.
        if !name.parse::<libc::pid_t>().is_ok_and(|pid| pid > 0) || owner_is_gone(&name) {
            let _ = std::fs::remove_file(entry.path());
            continue;
        }
        if let Some(at) = read_epoch(&entry.path()) {
            oldest = Some(at.min(oldest.unwrap_or(at)));
        }
    }
    oldest
}
/// Where the shell says a tracked command is running: ONE FILE PER INTERACTIVE
/// SHELL, named for that shell's pid, holding ONE EPOCH, the second the
/// command started. Written by pns for the interactive shell and removed when the
/// command ends; the sweep below reads and collects dead owners.
///
/// ONE FILE PER SHELL AND NOT ONE FILE. Every interactive shell on the machine
/// runs the same two bash-preexec functions, so a single shared path is a
/// marker any other pane erases: opening a tab, or running `ls` next door,
/// would delete a running build's evidence and leave this lamp dark for the
/// rest of that build. A directory makes each shell the only writer and the
/// only ordinary remover of its own file.
///
/// THE LONG TIER IS DERIVED FROM THAT EPOCH AND IS NOT A SECOND FIELD, because
/// it cannot be one. The marker is written when the command STARTS, and at
/// that instant the command has run for zero seconds, so nothing on the shell
/// side knows the tier yet; a flag would take a background timer rewriting the
/// file mid-command. `now - since` against the notifier's own threshold
/// answers the same question with one source of truth instead of two that can
/// disagree.
///
/// A SHELL KILLED MID-COMMAND LEAVES ITS FILE, and the pid in the NAME is what
/// collects it: the tick sweeps a marker whose process is gone, so a killed
/// terminal costs one tick's reading rather than a lamp breathing forever. The
/// lease stays the backstop for the case the pid cannot answer, a marker whose
/// shell is alive and whose command is not, because nothing renews the tick's
/// lease but a pns event.
pub(super) const LIGHTS_SHELL_DIR: &str = "lights-shell";
