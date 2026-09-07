use crate::*;

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
pub(crate) fn mark_present(decision: &pns::engine::Decision) {
    if !pns::missed_notifications::is_present(decision) {
        return;
    }
    let Some(now) = decision.inputs.now_secs else {
        return;
    };
    if read_epoch(&state_dir().join(LAST_PRESENT)).is_some_and(|held| held >= now) {
        return;
    }
    // NOTHING IS TAKEN AND NOTHING IS DELIVERED: the claim is asked for the
    // edge alone, and its answer is of no use here. What matters is that the
    // write happened inside it.
    let _ = claim_moment(Some(now), false);
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
fn advance_marker(now: u64) {
    let marker = state_dir().join(LAST_PRESENT);
    if read_epoch(&marker).is_some_and(|held| held >= now) {
        return;
    }
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = publish_state_line(&marker, &now.to_string());
}
/// One epoch off a state file, or None for anything this will not vouch for:
/// nothing at the path, a file that cannot be read, or text that is not a
/// plain count.
///
/// AN UNPARSEABLE MARKER IS NO EDGE AT ALL, never an edge at epoch zero. A
/// marker some other hand rewrote is not a near edge this can trust, and
/// reading one as zero would recap the whole ring.
pub(crate) fn read_epoch(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}
/// What one event found when it reached for the return moment.
///
/// ONE ARBITRATION OVER BOTH HALVES of what a return delivers, which is the
/// whole reason this is one value rather than a claim per file. The halves are
/// the recap card and the catch-up card, and with a claim each the loser of one
/// could still win the other: MEASURED at roughly one run in three with eight
/// racers, a racer that found the marker held read no window, fell through to
/// the journal, and put its catch-up card on the phone beside the winner's
/// recap card.
pub(crate) enum Moment {
    /// This event OWNS the moment. `since` is the near edge the marker held,
    /// absent when there was no marker to open a window with; `waiting` is the
    /// journal, claimed inside the same critical section.
    Owned {
        since: Option<u64>,
        waiting: Vec<pns::missed_notifications::Entry>,
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
/// NOTHING IS LEFT BEHIND on any path this run completes, and a run killed
/// mid-claim leaves ONE file that the next return adopts by name. The
/// adoption is also the recovery: the edge that run was holding comes back
/// with it rather than being lost.
pub(crate) fn claim_moment(now: Option<u64>, take_journal: bool) -> Moment {
    let state = state_dir();
    let marker = state.join(LAST_PRESENT);
    let claim = marker.with_extension(window_claim_suffix(now));
    let taken = if std::fs::rename(&marker, &claim).is_ok() {
        Some(claim)
    } else {
        match stranded_window_claim(&state, now) {
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
        claim_journal(&state)
    } else {
        Vec::new()
    };
    if let Some(edge) = since.max(now) {
        advance_marker(edge);
    }
    if let Some(claim) = taken {
        // The failure is dropped: what it leaves is exactly what the adoption
        // above recovers.
        let _ = std::fs::remove_file(claim);
    }
    Moment::Owned { since, waiting }
}
/// Whether another run is inside the return moment right now, and the claim it
/// left behind when it is not.
///
/// MATCHED ON THE MARKER'S OWN CLAIM PREFIX and nothing looser, which is
/// `stranded_claims`' rule: the journal and the turn marker claim themselves
/// in this directory too, and a wider match would hand one of their values
/// back as a window's near edge.
///
/// AT MOST ONE OF THESE CAN EXIST AT A TIME, because a claim is only ever made
/// by renaming the ONE marker or by renaming an existing claim, and a run that
/// finds one live makes none of its own. The loop still answers `Live` for the
/// first live one it meets rather than assuming that, because the directory is
/// a plain directory another hand can reach.
enum StrandedWindow {
    /// A run that still exists holds the marker.
    Live,
    /// A claim nobody is inside any more, and so the near edge it is holding.
    Abandoned(std::path::PathBuf),
    /// Nothing is holding anything: no marker was ever published here.
    None,
}
fn stranded_window_claim(state: &Path, now: Option<u64>) -> StrandedWindow {
    let prefix = format!("{LAST_PRESENT}.claim.");
    let Ok(entries) = std::fs::read_dir(state) else {
        return StrandedWindow::None;
    };
    let mut abandoned = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(owner) = name.strip_prefix(&prefix) else {
            continue;
        };
        if !window_claim_is_free(owner, now) {
            return StrandedWindow::Live;
        }
        abandoned = Some(entry.path());
    }
    abandoned.map_or(StrandedWindow::None, StrandedWindow::Abandoned)
}
/// What a window claim is named after the prefix: the id of the run that took
/// it, and the epoch it was taken at when that run had a clock to read.
///
/// THE EPOCH IS THE CLAIM'S OWN AGE and cannot be taken off the file instead:
/// a rename carries the marker's mtime, which is the time of the last PRESENT
/// event and can be hours before the claim was made. It costs nothing to
/// record, because the caller already holds this event's clock read.
fn window_claim_suffix(now: Option<u64>) -> String {
    match now {
        Some(now) => format!("claim.{}.{now}", std::process::id()),
        None => format!("claim.{}", std::process::id()),
    }
}
/// Whether a window claim may be taken: nobody is inside it.
///
/// THREE WAYS IT IS FREE, and the first two are `claim_by_rename`'s own. It is
/// THIS RUN'S, so nothing else can be inside it; or its owner has EXITED, so
/// nothing is; or it is far OLDER than any run could still be holding it.
///
/// THE AGE TEST IS WHAT A PID CANNOT ANSWER. A claim is held for two renames
/// and a small read, so a claim minutes old is one whose owner died mid-claim
/// and whose id the machine has since handed to something long-lived. Without
/// it that claim reads as live for as long as the new process runs, and every
/// return moment on the machine stands down behind it: no card, no recap and
/// no edge, until that process happens to exit. The bound is deliberately five
/// minutes, four orders of magnitude past what holding one costs, so a real
/// holder can never be stolen from and a stranded one can never wedge for long.
fn window_claim_is_free(owner: &str, now: Option<u64>) -> bool {
    let mut named = owner.split('.');
    let took_it = named.next().unwrap_or_default();
    if took_it == std::process::id().to_string() || owner_is_gone(owner) {
        return true;
    }
    match (named.next().and_then(|at| at.parse::<u64>().ok()), now) {
        (Some(taken), Some(now)) => now.saturating_sub(taken) > STALE_WINDOW_CLAIM_SECS,
        // A CLAIM WITH NO EPOCH, or a run with no clock to compare it against,
        // falls back on the pid alone, which is `abandoned_hold`'s own answer
        // and its own accepted price.
        _ => false,
    }
}
/// How long a window claim may stand before it is taken to be stranded
/// whatever its process id says. See `window_claim_is_free`.
const STALE_WINDOW_CLAIM_SECS: u64 = 300;
/// One line holding the epoch of the last event that PROVED the operator was
/// here, which is the near edge of the window a recap covers. Absent means no
/// window at all, so a fresh install cannot recap the whole ring.
const LAST_PRESENT: &str = "last-present";
