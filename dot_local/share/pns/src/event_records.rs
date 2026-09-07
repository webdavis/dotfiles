use crate::*;

/// Append one decision to the ring, and prune it back to the cap.
///
/// FAIL-QUIET, in `remember_staleness`'s style and deliberately the opposite
/// of `quiet_mode`'s loud write. A mute that did not land is a promise broken
/// to a human standing at the terminal; a decision that did not record is a
/// diagnostic missing later, on a path whose stdout is read by a harness hook
/// and whose only reader already says honestly that it has nothing. Printing a
/// complaint here would put a line about the state directory into every hook's
/// output for the rest of this machine's life.
pub(crate) fn record_decision(record: &pns::decision_log::Record) {
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = append_ring_line(
        &state_dir().join(DECISIONS),
        &pns::decision_log::line(record),
        pns::decision_log::KEPT,
        RING_READ_MAX,
    );
}
/// Journal one event the operator could not have perceived, so a replayer can
/// find it later. A delivered event writes nothing at all.
///
/// ITS OWN FUNCTION rather than a second job inside `record_decision`: the two
/// records have different reasons to change, and this write is conditional
/// where the decision's is not.
///
/// FAIL-QUIET, in `record_decision`'s exact style and for its exact reason. An
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and a journal entry that did not land costs a replay,
/// never a card.
///
/// THE EPOCH IS THE DECISION'S OWN CLOCK READ, taken off the readings it
/// decided from rather than by a second `SystemTime` call here: two readings
/// of one moment can disagree.
pub(crate) fn record_missed(
    event: &pns::args::EventArgs,
    decision: &pns::engine::Decision,
    overrides: &Overrides,
) {
    if !pns::missed_notifications::was_missed(decision, overrides) {
        return;
    }
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = append_ring_line(
        &state_dir().join(MISSED_NOTIFICATIONS),
        &pns::missed_notifications::entry(
            event,
            decision.inputs.now_secs,
            render::PREVIEW_MAX_CHARS,
        ),
        pns::missed_notifications::KEPT,
        RING_READ_MAX,
    );
}
/// Record one event in the activity ring, WHETHER OR NOT anybody perceived it.
///
/// THE THIRD FILE, and it exists because the two already here answer other
/// questions. The decision ring refuses free text by design, since a human
/// reads it through `pns doctor`; the journal is written only for events the
/// operator COULD NOT have perceived, which is the opposite of what a return
/// recap is about. The recap's window is the cards that WERE delivered,
/// glanced at and forgotten, and neither existing file can see one.
///
/// NEVER CLAIMED AND NEVER CONSUMED, unlike the journal. It is a rolling
/// window pruned by depth alone, which is what lets the detached recap child
/// re-read it safely and what makes a recap idempotent by WINDOW rather than
/// by deletion.
///
/// ITS OWN CAP AND ITS OWN READ CEILING, both stated on the constants. A recap
/// line is one of a hundred, so it is capped far shorter than a card, and the
/// depth that covers an overnight window needs a read ceiling of its own.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason: an
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and a missing entry costs one line of one recap.
///
/// THE PRIVACY RULE IS THE JOURNAL'S, INHERITED. This file holds the
/// operator's own text for every event, at 0600 like every other state file,
/// and nothing prints an entry to a terminal: `pns doctor` deliberately gains
/// no activity line, and the only reader is the recap that delivers it to the
/// same channels the live event reached.
pub(crate) fn record_activity(event: &pns::args::EventArgs, decision: &pns::engine::Decision) {
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = append_ring_line(
        &state_dir().join(ACTIVITY),
        &pns::missed_notifications::entry(event, decision.inputs.now_secs, ACTIVITY_MAX_CHARS),
        ACTIVITY_KEPT,
        ACTIVITY_READ_MAX,
    );
}
/// Every activity entry inside a window, oldest first, which is the order the
/// append leaves the ring in.
///
/// THE NEAR EDGE IS EXCLUSIVE and the far edge is not, which is the difference
/// between "since you were last here" and "including the moment you were".
/// MEASURED: with it inclusive, the event that MOVED the marker is counted
/// inside the next window, and every event sharing that same second with it is
/// too. Eight events in one second then read as a loud window opening at the
/// instant it closed, so a burst at the desk earned a recap of an absence that
/// never happened, and a second recap of the window a first one had just
/// posted. Excluding the marker's own second costs nothing real: the event at
/// that instant is the one that proved the operator was present.
///
/// AN ENTRY WITH NO CLOCK IS IN NO WINDOW. Its writer had no readable clock, so
/// nothing can place it, and counting it would put an event of unknown age
/// inside a bracket that is entirely about age.
///
/// A RING THAT CANNOT BE READ IS AN EMPTY WINDOW, which reads as no recap
/// rather than as a recap of nothing: the count would be zero, and zero is
/// under every threshold.
pub(crate) fn activity_in(since: u64, until: u64) -> Vec<pns::missed_notifications::Entry> {
    let Ok(contents) =
        pns::system::readable_state_file(&state_dir().join(ACTIVITY), ACTIVITY_READ_MAX)
    else {
        return Vec::new();
    };
    pns::missed_notifications::entries(&contents)
        .into_iter()
        .filter(|entry| entry.at.is_some_and(|at| at > since && at <= until))
        .collect()
}
/// The most of the ACTIVITY ring that is ever read into memory, which is its
/// own number because its depth is its own.
///
/// THE ARITHMETIC, in `KEPT`'s style so the next person to raise either number
/// has the ceiling in front of them. A worst-case entry is five text fields at
/// `ACTIVITY_MAX_CHARS` characters, each character costing six bytes escaped
/// (a control byte is written `\u001b`), plus about eighty bytes of JSON
/// scaffolding: 5 * 120 * 6 + 80 = 3,680 bytes. At `ACTIVITY_KEPT` that
/// MEASURES 552,000 bytes, which is 53% of this ceiling. Raising the depth or
/// the field cap means raising this in the same change, because a ring that
/// cannot be read back cannot be pruned and collapses to one line.
const ACTIVITY_READ_MAX: u64 = 1024 * 1024;
/// The decision ring: one line per event, `KEPT` deep, beside `quiet-until`
/// and `home-staleness`. NOT a log stream and not rotate-logs' business: it is
/// bounded state that prunes itself.
pub(crate) const DECISIONS: &str = "decisions";
/// The missed-notification journal: one JSON object per line, oldest first,
/// `missed_notifications::KEPT` deep, beside `decisions` and `quiet-until`.
/// Bounded state that prunes itself, not a log stream and not rotate-logs'
/// business.
pub(crate) const MISSED_NOTIFICATIONS: &str = "missed-notifications";
/// The activity ring: EVERY event, one JSON object per line in the journal's
/// own shape, oldest first, `ACTIVITY_KEPT` deep. Bounded state that prunes
/// itself, never claimed and never consumed.
const ACTIVITY: &str = "activity";
/// How many events the activity ring keeps.
///
/// A HUNDRED AND FIFTY covers an overnight window at the observed working rate
/// (ten pull requests merged in a ten-hour stretch on 2026-08-29, each spanning
/// many turns and so many events). Past that the ring under-reports its oldest
/// end exactly as the journal's prune does, which is why the recap's header
/// counts the entries it READ rather than claiming a total it cannot back.
/// Raising it means raising `ACTIVITY_READ_MAX` in the same change.
const ACTIVITY_KEPT: usize = 150;
/// How much of each text field one activity entry holds.
///
/// A TIMELINE LINE, NOT A CARD, which is why it is far under the card's own
/// 260: the recap renders one line per event among a hundred, and the full text
/// of every event already reached the durable log the recap's tail points at.
const ACTIVITY_MAX_CHARS: usize = 120;
