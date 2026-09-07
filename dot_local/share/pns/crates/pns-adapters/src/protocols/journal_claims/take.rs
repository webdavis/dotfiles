use super::*;
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
/// A claim already named for this process stays intact. The adapter test can
/// now plant this exact name; an unread earlier batch is never overwritten.
pub(super) fn claim_by_rename(journal: &Path) -> Claimed {
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
/// Hold by rename before reading. Completion, never this read, removes it.
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
/// An unreadable hold is never removed. Removing first, or removing whatever the read answered, throws
/// away a batch nobody has seen the moment the read fails: MEASURED as a
/// journal with one undecodable byte in it coming back empty, with the file
/// already gone. A read that failed leaves the held file exactly as it is, for
/// the adoption that recovers it.
pub(super) fn take_claim(claim: &Path) -> Claimed {
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
    let Ok(contents) = crate::readable_state_file(&held, RING_READ_MAX) else {
        return Claimed::LeftForAdoption;
    };
    Claimed::Taken(HeldJournal {
        entries: crate::journal_codec::entries(&contents),
        holds: vec![held],
    })
}

#[cfg(test)]
mod tests;
