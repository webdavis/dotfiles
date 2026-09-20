use crate::{publish_state_line, state_dir};
use std::path::Path;

/// Start or end this session's wait on the operator, which is what the blocked
/// lamp is derived from.
///
/// ONE FILE PER WAITING SESSION, named by the session id through
/// `lights::blocked_marker`, so a harness id that cannot be a filename writes
/// nothing at all rather than escaping the state directory.
///
/// EVERY EVENT ENDS A WAIT EXCEPT THE FOUR THAT START ONE, which is
/// `blocked_marker_action`'s rule and not a second copy of it here.
///
/// THE ANSWER ITSELF IS AN ARM NOW, per class. `PostToolUse` for
/// `AskUserQuestion` and `ExitPlanMode` is the answer, because for those two
/// the tool IS the dialog and it returns when the dialog is answered, and
/// `ElicitationResult` is an elicitation's answer; all three are declared to
/// `resolved`. `prompt` clears on the operator typing and `PostToolBatch` on
/// the batch coming back, and Stop is the last of the arms that get there.
/// THE LAG THAT IS LEFT, NAMED RATHER THAN HIDDEN: an ordinary tool approval
/// still has no answer event, so `resolved` at batch resolution is the
/// earliest clear for one, and a wait whose session produces no event at all
/// is bounded by the tick's own backstop.
///
/// STARTING ONE RIDES BEHIND THE `[lights]` TABLE, and ENDING ONE DOES NOT. A
/// machine that never asked for the lamps must not start accumulating files
/// about them, and nothing would ever sweep them there: the tick is the only
/// sweeper and it does not run without the table. Removal is one unlink with
/// nothing to accumulate, and gating it too meant a wait that ended while the
/// lamps were off kept its marker: switching hue back on inside the configured
/// backstop then put blocked on a lamp for a session nobody was waiting on.
///
/// AN END NEVER REMOVES A WAIT ARMED AFTER ITS OWN MOMENT, which is what
/// makes the unordered arms safe. Every clearing arm is asynchronous, so a
/// Stop still summarizing, or one question's own answer, can reach this line
/// after the next `PermissionRequest` published a second wait. The marker
/// holds the second it was armed and the caller states the moment it is
/// clearing for, so the compare keeps the newer file. The removal is OWNED BY
/// RENAME rather than read-then-unlink, in `sweep_markers`'s exact shape and
/// for its exact reason: concurrent unlink reports success to every caller on
/// this filesystem (see
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`), so the epoch
/// is read off the claim and a marker that turned out to be newer is put back
/// at its own path. A COLLISION INSIDE ONE SECOND STILL LOSES IT, because the
/// marker's unit is the second the backstop also reads; that residual is
/// closed by the session's next event, which re-publishes the wait it is
/// still in.
///
/// THE BACKSTOP CANNOT SWEEP A MARKER THE REMINDER HAS NOT YET NUDGED, and that is
/// held at CONFIG LOAD rather than here: `[lights.blocked] lease_expiry`
/// shorter than `[remind] delay` is refused by name (`config::parse_config`),
/// because it is a config that gives up on a wait before it ever nudges about
/// it. Nothing at this level re-publishes a swept marker, so nothing here has
/// to tell an abandoned session from a live one.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason.
pub fn update_blocked_marker(
    state_dir: &Path,
    session_id: &str,
    event_state: &str,
    lamps_live: bool,
    now: Option<u64>,
) {
    let Some(marker) = crate::marker_files::blocked_marker(state_dir, session_id) else {
        return;
    };
    match pns_domain::lights::phase::blocked_marker_action(event_state) {
        pns_domain::lights::phase::Action::Start if !lamps_live => {}
        pns_domain::lights::phase::Action::Start => {
            // THE DECISION'S OWN CLOCK, as record_news beside it: this reads
            // the moment the decision was made for, never a fresh one taken
            // inside this function. NO CLOCK IS NO MARKER, never a marker at
            // epoch zero: the bound that expires an abandoned wait is
            // measured against this number, and a zero would be expired the
            // moment it was written or, read the other way, would be a wait
            // nobody could age out.
            if let Some(now) = now {
                let _ = publish_state_line(&marker, &now.to_string());
            }
        }
        // The failure is DROPPED here and nowhere else: see the doc comment.
        pns_domain::lights::phase::Action::End => end_wait_at(&marker, now),
    }
}
/// End this session's wait on the operator directly: a state-only file move
/// in `clear_remind`'s style, with no event built, no config loaded and no
/// decision made.
///
/// TWO CALLERS NEED EXACTLY THIS, both in `hook_mode`: `prompt`, because the
/// operator answering a live wait by typing is not `resolved`'s signal
/// (PermissionRequest is decided off this hook's stdout, never off a later
/// PostToolBatch), and `resolved` itself, which carries every answer signal
/// the harness has and is guarded there against a subagent's own batch.
/// Ending is unconditional in the LAMP SWITCHES, unlike starting one: see
/// `update_blocked_marker`'s comment on why an End never checks them. It is
/// not unconditional in the CLOCK, and takes the caller's own moment for the
/// same compare the update's End branch makes.
pub fn end_blocked_wait(session_id: &str, now: Option<u64>) {
    if let Some(marker) = crate::marker_files::blocked_marker(&state_dir(), session_id) {
        end_wait_at(&marker, now);
    }
}

/// Remove one wait's marker, unless it was armed after the moment being
/// cleared for.
///
/// NO CLOCK IS AN UNCONDITIONAL REMOVAL, which is the asymmetry with the Start
/// beside it: a Start with no clock writes nothing, because the backstop is
/// measured against the number it would have written, while an End cannot be
/// withheld on a reading nobody has or a wait nothing answered would hold the
/// lamp for the whole backstop. AN UNREADABLE EPOCH IS REMOVED for
/// `sweep_markers`'s reason: nothing can ever age out a marker no reader will
/// vouch for.
fn end_wait_at(marker: &Path, now: Option<u64>) {
    let Some(now) = now else {
        let _ = std::fs::remove_file(marker);
        return;
    };
    let (Some(directory), Some(name)) = (marker.parent(), marker.file_name()) else {
        return;
    };
    let claim =
        crate::marker_files::sweep_claim(directory, &name.to_string_lossy(), std::process::id());
    if std::fs::rename(marker, &claim).is_err() {
        return;
    }
    match crate::marker_files::read_epoch(&claim) {
        // IT IS NEWER THAN THIS END, so a wait published between this
        // caller's moment and its claim is being held here. Put it back.
        Some(armed) if armed > now => {
            if std::fs::rename(&claim, marker).is_err() {
                let _ = std::fs::remove_file(&claim);
            }
        }
        _ => {
            let _ = std::fs::remove_file(&claim);
        }
    }
}

#[cfg(test)]
mod tests;
