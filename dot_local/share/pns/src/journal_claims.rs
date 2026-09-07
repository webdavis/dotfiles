use crate::*;

/// What became of one claim this run reached for.
///
/// FOUR OUTCOMES RATHER THAN ONE EMPTY VECTOR, because they are four different
/// things to have happened and only one of them may destroy anything. This
/// used to collapse into `Vec::new()`, and that is exactly how a journal whose
/// read failed came to be deleted with nothing delivered: the failure was
/// indistinguishable from an empty queue at the one call site that could still
/// have put it back.
enum Claimed {
    /// Nothing was there to claim, or another run took it first.
    Nothing,
    /// The path holds something this tool never wrote. Put back where it was
    /// found, and not read.
    Refused,
    /// This run OWNS these entries: it read them, and the claim they came from
    /// is gone, so no other run can deliver them too.
    Taken(Vec<pns::missed_notifications::Entry>),
    /// The claim could not be read, or could not be given up. It is STILL ON
    /// DISK, whole: under its claim name when the claim was never taken, or
    /// under a held name, which a return AFTER this process is gone adopts.
    LeftForAdoption,
}
impl Claimed {
    /// The entries this run may deliver, which is none for every outcome but
    /// one. Nothing else may be delivered: an unread claim is still on disk,
    /// and delivering from it as well would show the operator the same batch
    /// twice.
    fn entries(self) -> Vec<pns::missed_notifications::Entry> {
        match self {
            Claimed::Taken(entries) => entries,
            Claimed::Nothing | Claimed::Refused | Claimed::LeftForAdoption => Vec::new(),
        }
    }
}
/// The journal, CLAIMED and consumed: whatever an earlier run stranded is
/// adopted first, then the journal itself is renamed out of the way, read
/// through the one guarded reader, and given up only once that read worked.
///
/// NOTHING UNDELIVERED IS EVER DESTROYED, which is the property the whole
/// order below exists for. What this run cannot read, it leaves; what it
/// cannot give up, it leaves; what it leaves sits under its claim name or a
/// held name, and one of the returns that follow goes looking for both.
///
/// CLAIMED BY RENAME, which is `consume_turn_marker`'s idiom and is atomic:
/// two events racing each other cannot both take one journal, because only one
/// rename can win. A SECOND RENAME IS THE SECOND ARBITER, for a batch an
/// earlier run stranded: `take_claim` moves it on to a name carrying its own
/// process id before it reads a byte, so two runs that both reached one
/// stranded claim still cannot both deliver it. The unlink used to hold that
/// job and MEASURED it cannot: on macOS 26.2 (APFS) eight processes unlinking
/// ONE path were every one of them told they had succeeded.
///
/// ADOPTION IS HOW A LOST BATCH COMES BACK. A run killed between the rename
/// and the delivery, and a run whose read failed, both leave a claim behind;
/// before this nothing ever looked at one again, so the queue sat in the state
/// directory for good, and the doctor's count could not even see it, because
/// that count reads the journal's own name.
///
/// OLDEST FIRST: a stranded claim WAS the journal on an earlier return, so it
/// is older than anything in the file now, and the summary renders newest
/// first from the far end of what this returns.
///
/// AND ALL OF IT BEFORE ANY DELIVERY, which is unchanged. The entries are in
/// memory from the moment this returns, so a channel that hangs to its
/// deadline and takes the process with it leaves no claim behind; and a claim
/// left behind some other way is now recovered rather than lost.
///
/// THE RACE, stated: an append that opened the journal path before the rename
/// writes into the claimed inode, and is replayed or lost depending on which
/// side of the read it lands. That is ONE entry at a rare boundary, the same
/// bound `append_ring_line` already names and accepts.
pub(crate) fn claim_journal(state: &Path) -> Vec<pns::missed_notifications::Entry> {
    let mut waiting = Vec::new();
    for stranded in stranded_claims(state) {
        waiting.extend(take_claim(&stranded).entries());
    }
    waiting.extend(claim_by_rename(&state.join(MISSED_NOTIFICATIONS)).entries());
    waiting
}
/// Every claim an earlier run left in the state directory, oldest first, plus
/// every hold whose owner did not live to give it up.
///
/// MATCHED ON THE JOURNAL'S OWN CLAIM PREFIX and nothing looser: the turn
/// marker claims itself in this directory too, under its own name, and a
/// wider match would hand a turn's start time to the replayer. The one
/// addition is an ABANDONED HOLD, which is a stranded batch in every way that
/// matters here and is admitted only once the run that took it is gone.
///
/// SORTED BY WHEN THEY WERE LAST WRITTEN, which is the journal's own
/// timestamp: a rename does not touch it, so a claim still carries the moment
/// its last entry was appended. A time that cannot be read sorts oldest, which
/// costs an ordering and never a delivery.
fn stranded_claims(state: &Path) -> Vec<std::path::PathBuf> {
    let prefix = format!("{MISSED_NOTIFICATIONS}.claim.");
    let Ok(entries) = std::fs::read_dir(state) else {
        return Vec::new();
    };
    let mut found: Vec<(Option<SystemTime>, std::path::PathBuf)> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix) || abandoned_hold(&name)
        })
        // `DirEntry::metadata` does not traverse a symlink, matching the
        // append's and the reader's own refusal to judge one by its target.
        .map(|entry| {
            (
                entry
                    .metadata()
                    .ok()
                    .and_then(|found| found.modified().ok()),
                entry.path(),
            )
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}
/// Whether a name is a HELD file whose owner is gone.
///
/// A held file is a batch some run had taken and was reading when it died, in
/// a window one rename wide. Nothing else may touch one while its owner lives,
/// which is the whole reason the name sits outside the claim prefix: an owner
/// that is still reading cannot have its batch taken a second time.
fn abandoned_hold(name: &str) -> bool {
    name.strip_prefix(&format!("{MISSED_NOTIFICATIONS}.held."))
        .is_some_and(owner_is_gone)
}
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
pub(crate) fn owner_is_gone(owner: &str) -> bool {
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
/// The journal renamed out of the way, or the reason it was not.
///
/// VERIFIED AFTER THE RENAME AND NOT BEFORE. A check taken first is a check of
/// a path something else is still free to change between the look and the
/// move, and what the remove would then act on is whatever the rename actually
/// carried. So the rename decides, and the claim it produced is what gets
/// judged: anything that is not a regular file goes straight back to the
/// journal's own path, untouched and unread.
///
/// A RENAME BACK THAT FAILS LEAVES IT AT THE CLAIM PATH, which is a state
/// nothing here can improve on: the guarded reader refuses a non-regular file
/// without opening it, so a later adoption leaves it alone as well. It is
/// never read and never removed, which is the same promise the append makes
/// about a path it did not write.
///
/// THE PID GUARD BELOW IS NOT PINNED BY A TEST, and cannot be: no test can
/// plant a claim named for a process id the engine has not been given yet.
/// What it costs if it is ever wrong is one replay deferred to the next
/// return; what it buys is that a rename can never land on an undelivered
/// batch.
fn claim_by_rename(journal: &Path) -> Claimed {
    let claim = journal.with_extension(format!("claim.{}", std::process::id()));
    // NEVER RENAMED OVER A CLAIM THAT IS ALREADY THERE. The name carries this
    // process's id, so the only way one exists at this point is a run of the
    // same id whose batch the adoption above could not take (a pid the machine
    // reused, in practice), and a rename overwrites: the journal would land on
    // top of a batch nobody has seen. Both are left where they are, and the
    // next return tries both again.
    //
    // NOT A RACE, unlike the check this replaced at the journal's own path:
    // only the process holding this id writes this name, and it is this one.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return Claimed::LeftForAdoption;
    }
    if std::fs::rename(journal, &claim).is_err() {
        return Claimed::Nothing;
    }
    if !matches!(std::fs::symlink_metadata(&claim), Ok(found) if found.is_file()) {
        let _ = std::fs::rename(&claim, journal);
        return Claimed::Refused;
    }
    take_claim(&claim)
}
/// One claim HELD BY RENAME, then read and given up, in that order.
///
/// THE RENAME IS THE OWNERSHIP TEST, and the remove is no longer one. It used
/// to be, on the premise that only one of two runs reading a stranded claim
/// could unlink it. MEASURED on macOS 26.2 (APFS), that premise is false:
/// eight processes unlinking ONE path were every one of them told they had
/// succeeded, and two racing runs that both read one claim both delivered it
/// (reproduced twice in 1500 rounds). A rename does arbitrate, measured in the
/// same run: 40 rounds of eight racers, one winner every time.
///
/// THE HELD NAME IS OUTSIDE THE PREFIX THE ADOPTION SCAN MATCHES, so nothing
/// can take this batch a second time while it is being read. It comes back
/// into that scan only once the process named in it is gone.
///
/// THE READ STILL COMES BEFORE THE REMOVE, which is the older half of this and
/// unchanged. Removing first, or removing whatever the read answered, throws
/// away a batch nobody has seen the moment the read fails: MEASURED as a
/// journal with one undecodable byte in it coming back empty, with the file
/// already gone. A read that failed leaves the held file exactly as it is, for
/// the adoption that recovers it.
fn take_claim(claim: &Path) -> Claimed {
    // ONE HELD NAME PER CLAIM, not per process: pid then a per-run sequence.
    // A single per-process name coupled every stranded claim in a run to the
    // first one, and an UNREADABLE first claim then occupied the name, was
    // migrated to a fresh name by every later run's adoption, always sorted
    // oldest, and so STARVED every good batch behind it forever. The sequence
    // dissolves the coupling; the adoption parses the pid segment alone.
    static HELD_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let held = claim.with_file_name(format!(
        "{MISSED_NOTIFICATIONS}.held.{}.{}",
        std::process::id(),
        HELD_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    // The same refusal `claim_by_rename` makes about its own claim, for the
    // same reason: a rename OVERWRITES, and a batch this run has not delivered
    // must never be what it lands on.
    if std::fs::symlink_metadata(&held).is_ok() {
        return Claimed::LeftForAdoption;
    }
    if std::fs::rename(claim, &held).is_err() {
        return Claimed::Nothing;
    }
    let Ok(contents) = pns::system::readable_state_file(&held, RING_READ_MAX) else {
        return Claimed::LeftForAdoption;
    };
    if std::fs::remove_file(&held).is_err() {
        return Claimed::LeftForAdoption;
    }
    Claimed::Taken(pns::missed_notifications::entries(&contents))
}
