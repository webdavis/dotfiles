use super::*;
/// What one event found when it reached for the return moment.
///
/// ONE ARBITRATION OVER BOTH HALVES of what a return delivers, which is the
/// whole reason this is one value rather than a claim per file. The halves are
/// the recap card and the catch-up card, and with a claim each the loser of one
/// could still win the other: MEASURED at roughly one run in three with eight
/// racers, a racer that found the marker held read no window, fell through to
/// the journal, and put its catch-up card on the phone beside the winner's
/// recap card.
pub(super) enum Moment {
    /// This event OWNS the moment. `since` is the near edge the marker held,
    /// absent when there was no marker to open a window with; `waiting` is the
    /// journal, claimed inside the same critical section.
    Owned {
        since: Option<u64>,
        waiting: HeldJournal,
    },
    /// A run that still exists holds the moment right now, so this event is
    /// inside somebody else's return and has claimed nothing.
    Busy,
}
/// The return moment claimed: the window's near edge and the journal taken
/// together, and the edge handed straight back.
///
/// CLAIMED BY RENAME, which is `claim_by_rename`'s idiom for
/// `claim_by_rename`'s reason. Two events firing at once is ordinary here (a
/// Stop hook and the long-running notifier are a normal pair) and only one
/// rename can win. An unlink cannot stand in: MEASURED on macOS 26.2 (APFS),
/// eight processes unlinking one path were every one of them told they had
/// succeeded.
///
/// THE NEAR EDGE COMES OFF WHAT WAS CLAIMED, and that is the ordering this
/// whole function exists to get right. Reading the marker first and renaming
/// it afterwards claims whatever marker is there BY THEN, which is not the one
/// the window was counted from, because the winner republishes inside that
/// gap. Both racers then post the same window. Claiming first means a racer
/// that takes a republished marker counts the empty window that value opens
/// and correctly earns nothing.
///
/// THE JOURNAL IS TAKEN INSIDE THE SAME CRITICAL SECTION, before the edge goes
/// back. That is what makes a second card of ANY KIND impossible at one return
/// moment: a racer arriving while this run holds the marker is told `Busy` and
/// says nothing, and a racer arriving after the edge is restored finds the
/// queue already gone and has nothing to say either.
///
/// THE EDGE IS RESTORED IMMEDIATELY, before the window is counted and long
/// before anything is dispatched, so the marker's absence is bounded by two
/// renames rather than by a delivery. A kill at any instant then costs the one
/// in-flight recap and never a future window: the next present event finds an
/// edge to open one with.
///
/// AND IT ONLY EVER MOVES FORWARD. `advance_marker` is what publishes it, so
/// the newer of the claimed value and this event's own clock is what stands,
/// and a claim taken with no readable clock puts back exactly what it took.
///
/// The window claim is released here. Journal holds travel to the replay
/// owner and remain until it records that the attempt completed.
pub(super) fn claim_moment(state: &Path, now: Option<u64>, take_journal: bool) -> Moment {
    let marker = state.join(LAST_PRESENT);
    let claim = marker.with_extension(window_claim_suffix(now));
    let taken = if std::fs::rename(&marker, &claim).is_ok() {
        Some(claim)
    } else {
        match stranded_window_claim(state, now) {
            // A LIVE HOLDER IS THE ONLY THING THAT SILENCES AN EVENT HERE. No
            // claim at all is a machine that has never published a marker, and
            // that event still owes its catch-up card.
            StrandedWindow::Live => return Moment::Busy,
            // ADOPTED BY A SECOND RENAME, which is `take_claim`'s idiom: two
            // runs that both reach one stranded claim still cannot both take
            // it, because only one rename can win.
            StrandedWindow::Abandoned(left) => std::fs::rename(&left, &claim).ok().map(|()| claim),
            StrandedWindow::None => None,
        }
    };
    let since = taken.as_deref().and_then(read_epoch);
    let waiting = if take_journal {
        claim_journal(state)
    } else {
        HeldJournal::default()
    };
    if let Some(edge) = since.max(now) {
        advance_marker(state, edge);
    }
    if let Some(claim) = taken {
        // The failure is dropped: what it leaves is exactly what the adoption
        // above recovers.
        let _ = std::fs::remove_file(claim);
    }
    Moment::Owned { since, waiting }
}
