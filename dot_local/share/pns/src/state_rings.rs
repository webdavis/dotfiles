use crate::*;

/// Publish one line to a state file, atomically. The error is returned rather
/// than swallowed, so each caller states its own fail direction: a background
/// warning drops it, and a human waiting on a typed command hears about it.
///
/// PUBLISHED BY RENAME, the way the turn marker's claim is claimed further
/// down. A plain write truncates first, so a reader landing between the
/// truncate and the bytes sees an empty file, which every reader of these
/// files reads as no state at all. The pending path sits in the SAME
/// directory, because a rename across filesystems is not one, and it carries
/// this process's id so two runs publishing at once cannot share one.
pub(crate) fn publish_state_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let pending = path.with_extension(format!("new.{}", std::process::id()));
    // THE PENDING FILE CARRIES THE MODE, because the rename is what publishes
    // it: a prune that wrote its replacement at the umask's mode would undo
    // the one the append created the file with.
    let mut pending_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(&pending)?;
    // AND AGAIN AFTER THE OPEN, because `mode` above applies only when the
    // open CREATES the file. The pending path carries this process's own id,
    // so a run interrupted between the open and the rename leaves one for the
    // next run of that pid to REUSE, and a reused inode keeps whatever mode it
    // was made with until this narrows it. Set on the open HANDLE rather than
    // on the path, so nothing can be swapped in underneath between the two.
    pending_file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))?;
    pending_file.write_all(format!("{line}\n").as_bytes())?;
    if let Err(error) = std::fs::rename(&pending, path) {
        // Nothing half-written is left in the state directory for the next
        // run to trip over.
        let _ = std::fs::remove_file(&pending);
        return Err(error);
    }
    Ok(())
}
/// How many times an append waits for a ring's own lock before giving up
/// rather than risk the very race the lock exists to prevent.
///
/// A HANDFUL OF SHORT SLEEPS PAST WHAT THE CRITICAL SECTION ITSELF EVER
/// TAKES: the whole locked span is one small read, one rewrite and one
/// rename, so a live holder clears in microseconds. Giving up costs the ONE
/// event that could not get in, in `record_decision`'s own fail-quiet style;
/// it never risks publishing over a sibling's newer state, which is the loss
/// this lock exists to prevent.
const RING_LOCK_ATTEMPTS: u32 = 200;
/// How long a ring's own lock is believed before a holder that died on it is
/// read as an orphan. Long past any real critical section, so this only ever
/// fires for a crash, in `lights_tick_stale_secs`'s own style for its own
/// job.
const RING_LOCK_STALE_SECS: u64 = 5;
/// The path beside a ring's own that arbitrates between two processes
/// touching it at once.
fn ring_lock_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    std::path::PathBuf::from(name)
}
/// One ring's lock, WAITED FOR rather than skipped: unlike a lights tick,
/// which safely stands down from a busy window and picks the lamp up again
/// next interval, standing down here means silently losing whichever event
/// is mid-append. Reuses `claim_lock`, the one shape every lock in this
/// binary uses (see its own doc comment), rather than a second mechanism.
/// Bounded anyway, in this binary's own style: `RING_LOCK_ATTEMPTS` short
/// sleeps, and a hold that outlasts all of them is read as broken rather than
/// waited on forever.
fn claim_ring_lock(path: &Path) -> Option<HeldLock> {
    let lock = ring_lock_path(path);
    // A CLOCK THAT CANNOT BE READ COUNTS AS ZERO, which is `lock_aged_out`'s
    // own safe direction under a different name: a held lock is never read as
    // older than it is, so a broken clock can stand this caller down but
    // never lets it steal a live holder's claim.
    let now = now_secs().unwrap_or(0);
    for attempt in 0..RING_LOCK_ATTEMPTS {
        if claim_lock(&lock, now, RING_LOCK_STALE_SECS) {
            return Some(HeldLock(lock));
        }
        if attempt + 1 < RING_LOCK_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    None
}
/// The append and the prune behind it, for ANY of this tool's bounded state
/// rings. The caller names the file and its own depth; everything below is
/// one hardening serving every one of them, because a second hand-written
/// copy of it is how one ring ends up without the FIFO guard.
///
/// THE WHOLE OPERATION IS ONE CLAIM: append, read-back, prune and publish all
/// happen while this process alone holds the ring's own lock. Two events
/// firing at once (a Stop hook and the long-running notifier are a normal
/// pair) used to be safe only for the append itself; the prune's read and its
/// publish were NOT one atomic step, so a racer that read before a sibling's
/// append could still publish its stale, smaller window AFTER the sibling
/// published a newer one, silently dropping the sibling's line and keeping
/// the wrong oldest entry. The lock is what makes the four steps indivisible,
/// which is also what retires the old accepted limit below: an append can no
/// longer land during a sibling's rename, because no sibling is ever inside
/// this section at the same time.
///
/// NOTHING ABOUT THE FILE IS TRUSTED, because none of it is this tool's word:
/// the ring is a plain file in a directory an operator, a backup tool or
/// another program can reach. Three states were MEASURED to cost more than
/// the record they lost. A FIFO at the path parks the open forever, and with
/// it the hook that called this, on every event. A byte no reader can decode
/// fails the read-back, which is what the prune runs on, so the ring then
/// grows without a bound. A file left without its trailing newline welds this
/// record onto the tail of the last one and costs the reader BOTH. Each is
/// answered here rather than defended against downstream: an irregular file
/// is refused untouched, and a file this cannot read back whole is replaced
/// by the one line it does have.
///
/// `read_max` IS THE CALLER'S TOO, and it travels with `kept` because the two
/// are one decision. The prune runs on the READ-BACK, so a ring deep enough to
/// exceed the reader's ceiling can never be pruned again: the heal fires and
/// the file collapses to the one line just written, silently, exactly when it
/// is fullest. Every caller states both numbers together, and the doc comment
/// on each depth does the arithmetic.
pub(crate) fn append_ring_line(
    path: &Path,
    line: &str,
    kept: usize,
    read_max: u64,
) -> std::io::Result<()> {
    // BEFORE THE CLAIM: the lock lives beside the ring, so a state directory
    // that does not exist yet fails the lock's own exclusive create with the
    // same `NotFound` the ring's own open used to paper over here. A first
    // event has nowhere else to make this directory.
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Some(_lock) = claim_ring_lock(path) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "the ring's lock stayed held past every attempt",
        ));
    };
    // BEFORE THE OPEN, and with `symlink_metadata` so the link itself is what
    // is judged rather than whatever it points at. Refused and never
    // repaired: deleting something this tool did not put there, on a path it
    // only ever appends to, is a bigger action than skipping one record.
    let already_there = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => true,
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the ring is not a regular file",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    // The separator rides IN the same write rather than being a write of its
    // own, so the record still lands in one append and two events racing each
    // other still cannot interleave.
    let separator = if already_there && ends_mid_line(path)? {
        "\n"
    } else {
        ""
    };
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(STATE_FILE_MODE)
        .open(path)?
        .write_all(format!("{separator}{line}\n").as_bytes())?;

    let contents = match pns::system::readable_state_file(path, read_max) {
        Ok(contents) => contents,
        // THE HEAL. What could not be read back cannot be pruned either, so
        // leaving it would leave the ring unbounded from here on. The line
        // just written is the part that is known good and known this tool's
        // own, and it is republished alone.
        Err(error) if republish_after(&error) => return publish_state_line(path, line),
        // AND NOT WHEN THE PATH IS SIMPLY GONE. Nothing removes one of these
        // files except a claim, and a claim is a rename: the file this append
        // just wrote into moved to the claim path AND TOOK THIS LINE WITH IT,
        // on its way to being delivered. Republishing it here would put a
        // second copy of an already-claimed record back at the path, and the
        // operator would be shown it twice. There is nothing left to prune, so
        // there is nothing to do.
        Err(_) => return Ok(()),
    };
    // A TEST-ONLY STALL, in `env_deadline`'s own words: it exists so a test
    // can prove this section is exclusive rather than hope a real race lands
    // in a window that is normally microseconds wide. Unset in every real
    // invocation, so production takes no delay here at all.
    if let Some(delay) = env_deadline("PNS_RING_LOCK_TEST_DELAY_MS") {
        std::thread::sleep(delay);
    }
    let entries: Vec<&str> = contents.lines().collect();
    if entries.len() <= kept {
        return Ok(());
    }
    // Joined with newlines, because the publish writes the one trailing
    // newline back itself.
    publish_state_line(path, &entries[entries.len() - kept..].join("\n"))
}
/// Whether an append whose read-back FAILED has to republish the line it just
/// wrote.
///
/// EVERY REASON BUT ONE. A file that cannot be decoded, is too large to read,
/// or is no longer a regular file is a ring that can never be pruned again, so
/// the one line known to be this tool's own is republished over it. NotFound
/// is the exception and the only one: these files are removed by nothing but a
/// claim, and a claim is a rename, so an absent path means the line just
/// written is already inside the claim and on its way to the operator.
///
/// ITS OWN FUNCTION so the distinction can be stated in a test. The wiring
/// from a real interleaved claim into this arm is a race no test in this tree
/// can stage deterministically; what is pinned here is the decision, and the
/// race itself belongs to the out-of-tree probe.
fn republish_after(error: &std::io::Error) -> bool {
    error.kind() != std::io::ErrorKind::NotFound
}
/// Whether the ring's last byte is anything other than a newline, which is
/// what would FUSE the next record onto the entry already there.
///
/// READ-ONLY AND ON ITS OWN HANDLE, so the handle that writes stays
/// write-only. The end is found by seeking rather than taken from the size
/// the caller already read: another event can append between the two, and an
/// offset from the stale size would sample a byte out of the middle.
fn ends_mid_line(path: &Path) -> std::io::Result<bool> {
    let mut ring = std::fs::File::open(path)?;
    let end = ring.seek(std::io::SeekFrom::End(0))?;
    if end == 0 {
        return Ok(false);
    }
    ring.seek(std::io::SeekFrom::Start(end - 1))?;
    let mut last = [0u8; 1];
    ring.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}
/// The most of the decision ring or the journal that is ever read into memory.
/// Their depths (5 and 25) at their field caps sit far under it; see
/// `missed_notifications::KEPT` for that arithmetic.
pub(crate) const RING_READ_MAX: u64 = 256 * 1024;
/// The mode every file this tool creates in its state directory is born with.
///
/// ONE RULE FOR THE DIRECTORY'S CONTENTS rather than a knob for one caller:
/// none of them has a reason to be world-readable, and the journal holds the
/// operator's own text. ACCEPTED LIMIT: an APPEND applies it at create, so a
/// ring an earlier build already left on disk keeps its umask mode until it is
/// next created, and nothing chmods a file it found there, in keeping with the
/// ring's refuse-rather-than-repair stance. THE PUBLISH IS THE ONE PLACE THAT
/// CHMODS, and it is not that case: the pending file it narrows is its own,
/// named for this process, and the rename is about to publish that file's mode
/// over the state file.
pub(crate) const STATE_FILE_MODE: u32 = 0o600;

#[cfg(test)]
#[path = "state_rings/tests.rs"]
mod state_rings_tests;
