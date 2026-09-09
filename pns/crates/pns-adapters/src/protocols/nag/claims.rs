use crate::claim_lock;
use std::path::Path;
/// Every file in the nag directory that could be a record, sorted so a fire is
/// deterministic.
///
/// THE SUFFIX IS THE WHOLE FILTER, which is what keeps a claim out of this: a
/// held claim is `<name>.claim.<pid>` and can never end in the record suffix,
/// so a record another process is mid-fire on is never re-enumerated here.
pub fn record_entries(directory: &Path) -> Vec<std::path::PathBuf> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|entry| {
            entry
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(super::RECORD_SUFFIX))
        })
        .collect();
    entries.sort();
    entries
}

/// One record taken by rename, or None when somebody else has it.
///
/// THE RENAME IS THE OWNERSHIP TEST, in `consume_turn_marker`'s exact shape and
/// for `take_claim`'s measured reason: a plain unlink reports success to EVERY
/// racer on APFS, so a remove could tell two processes they each own this
/// record.
///
/// NOT THE SAME GUARANTEE AS THE FIRE CLAIM, and not made redundant by it. The
/// fire claim is what stops two processes carding in one window; this is what
/// stops ONE approval being counted twice when a second process is legitimately
/// running, which is what happens after a crashed fire's window claim ages out
/// while its records are still on disk. Independent record owners are tested
/// without the fire lock, so the rename's own arbitration is observable.
///
/// AN IRREGULAR FILE GOES BACK WHERE IT WAS AND IS NEVER OPENED, following
/// `append_ring_line`'s own refusal at a state path: a FIFO here would park the
/// read forever. The rename is still what tests it, because only the winner is
/// entitled to look at all.
pub fn claim_record(record: &Path) -> Option<std::path::PathBuf> {
    claim_record_as(record, std::process::id())
}

fn claim_record_as(record: &Path, owner: u32) -> Option<std::path::PathBuf> {
    let claim = super::claim_path(record, owner);
    // NEVER RENAMED OVER A CLAIM ALREADY THERE, for `claim_by_rename`'s reason:
    // the name carries this process's id, so anything sitting at it is a record
    // this pid claimed and could not finish, and a rename would land the new one
    // on top of it.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return None;
    }
    std::fs::rename(record, &claim).ok()?;
    if !matches!(std::fs::symlink_metadata(&claim), Ok(found) if found.is_file()) {
        let _ = std::fs::rename(&claim, record);
        return None;
    }
    Some(claim)
}

/// The whole fire owned ONCE, or None when this process is not the one holding
/// this window.
///
/// NOT A DUPLICATE OF THE PER-RECORD CLAIM, which answers a different question.
/// That one is per-approval crash safety: it is what stops one record being
/// counted by two processes, and it stays. But ownership taken per record lets
/// two woken processes each win a DISJOINT, NON-EMPTY subset and each card its
/// own true count, which is one card per FIRE rather than one card per fire
/// WINDOW, and that is precisely what the coalescing ruling forbids. Measured
/// on the build before this: sixteen concurrent fires over one directory
/// produced sixteen cards. The window is what has to be owned, so it is.
///
/// AN EXCLUSIVE CREATE IS THE ARBITRATION, NOT A RENAME, and the difference is
/// measured rather than stylistic. A rename claim moves the contended name OUT
/// of the way: the winner renames `fire.lock` to its own claim, so a racer that
/// looked for a holder a moment earlier finds no lock at that name, creates one
/// and takes it too. That form delivered TWO cards from four concurrent fires,
/// reproducibly, under load. An exclusive create leaves the lock sitting at its
/// name for the whole fire, so every later racer is refused by the same atomic
/// operation, whenever it arrives. The rename survives below, in the one place
/// a remove would be unsafe.
///
/// AND AGED OUT AT A MINUTE, so a crash mid-fire cannot wedge the feature for
/// good. A minute is a wide margin over the work the lock has to cover: the
/// holder claims every record by rename before it delivers anything, so a fire
/// that broke in later finds an empty directory in any case. What the wait
/// costs when the holder really did die is one nudge window, which is the safe
/// direction.
pub fn claim_fire(directory: &Path, now: u64) -> Option<std::path::PathBuf> {
    let lock = directory.join(super::FIRE_LOCK);
    claim_lock(&lock, now, super::FIRE_STALE_SECS).then_some(lock)
}
/// The fire given up, so the next window can be claimed without waiting out
/// `FIRE_STALE_SECS`.
///
/// SAID WHEN IT FAILS, and the consequence is named rather than implied: the
/// feature is not broken by a claim left behind, it is DELAYED, because the age
/// test is what recovers it.
pub fn release_fire(fire: &Path) {
    if let Err(error) = std::fs::remove_file(fire) {
        eprintln!(
            "pns nag: the fire claim {} could not be given up ({error}); the next fire waits it out",
            fire.display()
        );
    }
}

#[cfg(test)]
mod tests;
