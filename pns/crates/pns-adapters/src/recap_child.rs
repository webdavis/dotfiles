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
/// AND IT IS THE CHILD'S STDIN, because that child owns dispatching the card.
/// The pipe is the whole hand-off: `hand_recap_card` writes one line down it
/// and closes it, and a close with nothing written is the EOF that tells the
/// child there is no card. `--card-on-stdin` is what makes reading that pipe
/// legible in the child rather than inferred, and it is passed on every spawn,
/// since whether a card follows is not known until the spawn has answered.
///
/// A CHILD THAT DIES COSTS ONE RECAP, AND THE CARD IT TOOK: the activity ring
/// is not consumed and the marker has already moved. The recap arms its own
/// finite lifetime before reading sources; that owner survives this producer's
/// exit or group termination.
pub fn spawn_recap(since: u64, until: u64) -> Option<std::process::ChildStdin> {
    let binary = std::env::current_exe().ok()?;
    let mut child = Command::new(binary);
    child
        .args(["recap", "--since-epoch", &since.to_string()])
        .args(["--until-epoch", &until.to_string()])
        .args(["--to", DURABLE])
        .arg(CARD_ON_STDIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // A NEW GROUP, WITH ITS OWN ID, which is what `setpgid(0, 0)` in the
        // forked child does and what the doc above promises.
        .process_group(0);
    child.spawn().ok()?.stdin.take()
}

/// Hand the composed card to a started recap child, and say whether it took it.
///
/// ONE WRITE AND A CLOSE, never a wait: the payload is one short line and the
/// pipe's buffer swallows it whole, so this process is not made to queue behind
/// a child that is already rendering. Taking it is the strongest thing a writer
/// can be told; whether the child then delivers the card is the child's to
/// report, on the same route every other delivery reports on.
///
/// A REFUSED WRITE LEAVES THE CARD WITH THE CALLER, which is what makes the
/// hand-off safe: a child that died between the spawn and this call answers
/// `EPIPE` here, and the caller delivers the card itself.
pub fn hand_recap_card(
    mut stdin: std::process::ChildStdin,
    card: &pns_application::ReplayCard<'_>,
) -> bool {
    std::io::Write::write_all(&mut stdin, crate::recap_card_wire::encode(card).as_bytes()).is_ok()
}

/// The destination the return moment's recap is delivered to, named on the
/// child's own argv rather than assumed inside it.
///
/// THIS IS THE FOLD. The return card and a hand-typed `pns recap --to ...`
/// now take one code path: the child asks for a destination the way an
/// operator does, so the card's Discord message and the terminal page cannot
/// drift apart. It names the ROLE rather than the transport, because
/// `[plugins.log] type` decides which of hermes and Discord carries the paper
/// trail and the child reads no config.
pub const DURABLE: &str = "durable";

/// The word that tells a recap child its stdin carries a card, or the EOF that
/// says there is none. NEVER in the usage text: the event path passes it and an
/// operator running a recap by hand has no card to hand over.
pub const CARD_ON_STDIN: &str = "--card-on-stdin";
/// How long a detached recap may live. Generous, because nobody is waiting on
/// this process; finite, because nobody is watching it either, and it holds
/// whatever `[delivery] remote_deadline` allows one call inside it.
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
