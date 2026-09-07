use crate::{RING_READ_MAX, append_ring_line};
use std::path::Path;

/// How many received `policy_settings` changes the audit trail remembers,
/// comfortably past the five-entry decision ring (`decision_log::KEPT`): a
/// policy change is rarer and more consequential than an ordinary observed
/// event, and it must outlive more than a handful of intervening turns rather
/// than vanish with them the moment the ring rolls over.
///
/// THE ARITHMETIC `append_ring_line` ASKS EVERY CALLER FOR, against the
/// `RING_READ_MAX` this passes beside it: a line is a timestamp, a session cut
/// to `CONFIG_SESSION_MAX_CHARS` and a path cut to `CONFIG_PATH_MAX_CHARS`, so
/// its worst case is about 4.4 KB of UTF-8 and twenty of them about 88 KB,
/// comfortably inside the reader's 256 KiB ceiling. Without both cuts the
/// depth alone would not bound the FILE, and a ring past that ceiling can
/// never be pruned again: the heal fires and the trail collapses to one line.
const POLICY_SETTINGS_AUDIT_KEPT: usize = 20;
/// The policy-settings audit trail's file name, beside `DECISIONS` and
/// `ACTIVITY`.
const POLICY_SETTINGS_AUDIT: &str = "policy-settings-audit";
/// Append one received `policy_settings` change to a bounded, state-only
/// audit record, so it outlives the five-entry decision ring an ordinary
/// observed event is logged to. STATE-ONLY, in `record_missed`'s style: no
/// card of its own, no marker, no lease; the routing this rides beside stays
/// marker-neutral, and this is purely a durable trace of receipt for a class
/// of change worth remembering past the next few turns.
///
/// FAIL-QUIET, in `record_decision`'s exact style and for its exact reason:
/// an event path whose stdout a harness hook reads must not gain a line about
/// the state directory, and a record that did not land costs a read of this
/// file later, never a card.
pub fn record_policy_settings_change(state: &Path, session: &str, path: &str, now: Option<u64>) {
    let now = now.unwrap_or_default();
    let path = if path.is_empty() { "none" } else { path };
    let line = format!("{now} session={session} file={path}");
    let _ = append_ring_line(
        &state.join(POLICY_SETTINGS_AUDIT),
        &line,
        POLICY_SETTINGS_AUDIT_KEPT,
        RING_READ_MAX,
    );
}
