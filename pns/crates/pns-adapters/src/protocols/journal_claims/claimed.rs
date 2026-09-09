use super::*;

/// What became of one claim this run reached for.
///
/// FOUR OUTCOMES RATHER THAN ONE EMPTY VECTOR, because they are four different
/// things to have happened and only one of them may destroy anything. This
/// used to collapse into `Vec::new()`, and that is exactly how a journal whose
/// read failed came to be deleted with nothing delivered: the failure was
/// indistinguishable from an empty queue at the one call site that could still
/// have put it back.
pub(super) enum Claimed {
    /// Nothing was there to claim, or another run took it first.
    Nothing,
    /// The path holds something this tool never wrote. Put back where it was
    /// found, and not read.
    Refused,
    /// This run owns the entries and keeps their file until the replay completes.
    Taken(HeldJournal),
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
    pub(super) fn entries(self) -> HeldJournal {
        match self {
            Claimed::Taken(entries) => entries,
            Claimed::Nothing | Claimed::Refused | Claimed::LeftForAdoption => {
                HeldJournal::default()
            }
        }
    }
}
/// Claim earlier stranded batches, then the current journal. Successful reads
/// stay under the live owner's held names until a replay attempt completes.
///
/// Unreadable claims stay on disk. A completed replay attempt consumes its
/// readable claims even when delivery failed, preserving the existing policy.
/// A crash or unwind before completion leaves the holds for later adoption.
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
/// Reading finishes before delivery. The held files remain throughout it, so
/// the loss window ends only when the attempt has returned its outcomes.
/// Durable outcome recording and retry policy belong to the delivery ledger.
///
/// THE RACE, stated: an append that opened the journal path before the rename
/// writes into the claimed inode, and is replayed or lost depending on which
/// side of the read it lands. That is ONE entry at a rare boundary, the same
/// bound `append_ring_line` already names and accepts.
pub fn claim_journal(state: &Path) -> HeldJournal {
    let mut waiting = HeldJournal::default();
    for stranded in stranded_claims(state) {
        waiting.extend(take_claim(&stranded).entries());
    }
    waiting.extend(claim_by_rename(&state.join(MISSED_NOTIFICATIONS)).entries());
    waiting
}

#[derive(Default)]
pub(crate) struct HeldJournal {
    pub(crate) entries: Vec<pns_domain::missed::Entry>,
    pub(crate) holds: Vec<std::path::PathBuf>,
}

impl HeldJournal {
    fn extend(&mut self, batch: Self) {
        self.entries.extend(batch.entries);
        self.holds.extend(batch.holds);
    }
}
