use super::*;

/// Move the recap window's near edge to this event, when this event proves the
/// operator was here.
///
/// THE EVENTS THE RETURN MOMENT NEVER REACHES, and only those in practice:
/// a muted event, an event whose plan decorated nothing because the operator
/// was watching the pane it came from, an event that found the moment held.
/// `claim_moment` moves the edge for every event that does reach it, at the
/// instant it takes the claim, so the read below is already satisfied by the
/// time this runs on those.
///
/// AND THROUGH THE SAME CLAIM, which is not decoration. MEASURED at one run in
/// sixty with eight racers: a run that found the moment held republished the
/// marker here anyway, out from under the holder, and a third run then renamed
/// that fresh marker and became a SECOND owner alongside the first. The two
/// then raced on the journal, and the pair of them put a recap card and a
/// catch-up card on the phone at one moment. Nothing may publish this path
/// while somebody holds it.
///
/// THE READ IN FRONT OF THE CLAIM IS AN OPTIMISATION AND ALSO THE POINT. An
/// edge already at or past this event needs no write, so the ordinary event
/// takes no claim at all and cannot make a racer defer its card; and a marker
/// that is ABSENT reads as None here, which correctly falls through to the
/// claim, where the holder is found and this run stands down.
///
/// AFTER THE CARD SITE, and the ordering is the whole idempotence rule. The
/// window a recap covers ends where this event is, so moving the edge before
/// `replay_missed` counted the window would leave every count at one and no
/// recap could ever fire.
///
/// THE EPOCH IS THE DECISION'S OWN CLOCK READ, taken off the readings it
/// decided from rather than by a second `SystemTime` call, for the reason
/// `record_missed` states: two readings of one moment can disagree.
/// The calling use case checks presence before requesting this write.
pub fn mark_present(now: Option<u64>) {
    let Some(now) = now else {
        return;
    };
    if read_epoch(&state_dir().join(LAST_PRESENT)).is_some_and(|held| held >= now) {
        return;
    }
    // NOTHING IS TAKEN AND NOTHING IS DELIVERED: the claim is asked for the
    // edge alone, and its answer is of no use here. What matters is that the
    // write happened inside it.
    let _ = claim_moment(&state_dir(), Some(now), false);
}
/// The window's near edge published, and only ever FORWARD.
///
/// READ, COMPARE, PUBLISH. MEASURED as the reason: a slow event that read
/// epoch 100 and a quick one that read 101 both publish at the end of their
/// own run, so the slow one used to land last and put the edge back to 100.
/// Everything the quick event covered then reads as absence activity on the
/// next return, and a long enough tail of it crosses the threshold and posts a
/// recap of a window that never happened.
///
/// CALLED ONLY FROM INSIDE A CLAIM, which is what makes the read and the
/// publish safe as a pair: the caller holds the marker, so nothing else is
/// writing this path between them.
///
/// FAIL-QUIET, in `record_missed`'s exact style. A marker that did not land
/// costs one window's near edge, which the next present event moves anyway.
pub(super) fn advance_marker(state: &Path, now: u64) {
    let marker = state.join(LAST_PRESENT);
    if read_epoch(&marker).is_some_and(|held| held >= now) {
        return;
    }
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = publish_state_line(&marker, &now.to_string());
}
/// One line holding the epoch of the last event that PROVED the operator was
/// here, which is the near edge of the window a recap covers. Absent means no
/// window at all, so a fresh install cannot recap the whole ring.
pub(super) const LAST_PRESENT: &str = "last-present";
