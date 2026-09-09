use crate::remote_deadline;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

/// Start the recap in a process of its own, and say whether it really started.
///
/// THE DIGEST NEVER RUNS IN THIS PROCESS. `run_event` is reached from
/// `pns hook prompt`, which the harness does NOT background, and from the
/// bashrc notifier, where a human is watching their prompt. Rendering and
/// posting a recap sits on neither. NEVER WAITED ON, so this process exits
/// exactly when it would have, and the child is reparented if it goes first.
///
/// AND IN A PROCESS GROUP OF ITS OWN, which is the other half of detachment
/// and used to be claimed rather than done. A hook the harness times out is
/// killed by GROUP, and so is a shell prompt taking `SIGINT`; a child left in
/// the parent's group goes with it, after the marker has already moved on, so
/// the window can never fire again and the card in the operator's hand points
/// at a recap nobody is writing.
///
/// `current_exe` RATHER THAN A PATH, so a test binary re-execs itself and a
/// moved install still works. ONLY THE TWO BOUNDS CROSS: the child re-reads the
/// ring itself, so nothing is serialized between them and nothing is lost if
/// the child never starts.
///
/// TWO INDEPENDENT READS OF ONE RING, STATED. The card's count is this
/// process's own read of the window and the recap's header is the child's, so
/// an event landing in the shared `until` second between them, or a prune, can
/// leave the two counts one apart. Each is honest about what IT read, which is
/// the same rule the header's own comment states about the ring's depth;
/// reconciling them would mean serializing a snapshot the child is deliberately
/// free to re-read.
///
/// THE ANSWER IS WHETHER A CHILD EXISTS, which is what the card says out loud.
/// A spawn that failed must never leave a card pointing at a recap nobody is
/// writing.
///
/// A CHILD THAT DIES COSTS ONE RECAP AND NOTHING ELSE: the activity ring is
/// not consumed, the marker has already moved, and the card carried the counts.
/// The recap arms its own finite lifetime before reading sources; that owner
/// survives this producer's exit or group termination.
pub fn spawn_recap(since: u64, until: u64) -> bool {
    let Ok(binary) = std::env::current_exe() else {
        return false;
    };
    let mut child = Command::new(binary);
    child
        .args(["recap", "--since", &since.to_string()])
        .args(["--until", &until.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // A NEW GROUP, WITH ITS OWN ID, which is what `setpgid(0, 0)` in the
        // forked child does and what the doc above promises.
        .process_group(0);
    // AN UNBOUNDED DEADLINE IS A TERMINAL'S CHOICE, NEVER A BACKGROUND
    // CHILD'S. `PNS_REMOTE_TIMEOUT=0` is curl's `-m 0`, no deadline at all,
    // which nobody is behind to interrupt here: a wedged gateway would keep
    // this process alive for good, and every later window would add another.
    if remote_deadline(std::env::var("PNS_REMOTE_TIMEOUT").ok().as_deref()).is_none() {
        child.env("PNS_REMOTE_TIMEOUT", RECAP_DEADLINE_SECS.to_string());
    }
    child.spawn().is_ok()
}
/// The deadline a detached recap falls back to when the environment asked for
/// none. Generous, because nobody is waiting on this process; finite, because
/// nobody is watching it either.
const RECAP_DEADLINE_SECS: u64 = 30;

/// Bound the complete recap operation, including work before summarization.
pub fn run_recap_bounded(operation: impl FnOnce() -> i32) -> i32 {
    recap_with_deadline(
        std::time::Instant::now() + std::time::Duration::from_secs(RECAP_DEADLINE_SECS),
        operation,
    )
}

fn recap_with_deadline(expires_at: std::time::Instant, operation: impl FnOnce() -> i32) -> i32 {
    let Ok(_lifetime) = crate::process::Group::for_recap(expires_at) else {
        return 1;
    };
    operation()
}

#[cfg(test)]
#[path = "recap_child/tests.rs"]
mod tests;
