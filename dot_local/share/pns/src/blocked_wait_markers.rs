use crate::*;

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
/// THE LAG, NAMED RATHER THAN HIDDEN: the marker clears at the NEXT event from
/// that session, never at the instant the operator answered, because no event
/// reports the answer itself. STOP IS THE LAST OF THE ARMS THAT GET THERE, not
/// the only one: `prompt` clears on the operator typing and `resolved` on the
/// tool batch coming back, and each is why the two arms carry a comment of
/// their own. The worst case left is a wait whose session produces neither
/// before its turn ends, and the SUBAGENT RESIDUAL, which `resolved` skips by
/// design and which therefore does hold blocked until the parent's own Stop. The
/// tick's own bound is what stops an abandoned session holding it forever, and
/// the day item 21's rebuild wires a real answered signal this consumes it at
/// the same call site.
///
/// STARTING ONE RIDES BEHIND THE `[lights]` TABLE, and ENDING ONE DOES NOT. A
/// machine that never asked for the lamps must not start accumulating files
/// about them, and nothing would ever sweep them there: the tick is the only
/// sweeper and it does not run without the table. Removal is one unlink with
/// nothing to accumulate, and gating it too meant a wait that ended while the
/// lamps were off kept its marker: switching hue back on inside the configured
/// backstop then put blocked on a lamp for a session nobody was waiting on.
///
/// THE OLDER STOP CAN REMOVE THE NEWER WAIT'S MARKER, and that is a stated
/// limit rather than a rule. One file per SESSION carries no generation, so a
/// blocked event that publishes a new wait while the previous Stop is still
/// condensing loses it when that Stop reaches this line. Unlink cannot
/// arbitrate on this filesystem (see
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`), so telling the
/// two apart would need a generation IN the marker and a compare-and-swap
/// publish over it. The damage is bounded by the backstop above and closed by
/// the session's next event, which re-publishes the wait it is still in.
///
/// THE BACKSTOP CANNOT SWEEP A MARKER THE NAG HAS NOT YET NUDGED, and that is
/// held at CONFIG LOAD rather than here: `[lights.blocked] give_up_after_secs`
/// shorter than `[nag] after_secs` is refused by name (`config::parse_config`),
/// because it is a config that gives up on a wait before it ever nudges about
/// it. Nothing at this level re-publishes a swept marker, so nothing here has
/// to tell an abandoned session from a live one.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason.
pub(crate) fn update_blocked_marker(
    state_dir: &Path,
    session_id: &str,
    event_state: &str,
    lamps_live: bool,
    now: Option<u64>,
) {
    let Some(marker) = pns::lights::blocked_marker(state_dir, session_id) else {
        return;
    };
    match pns::lights::blocked_marker_action(event_state) {
        pns::lights::Action::Start if !lamps_live => {}
        pns::lights::Action::Start => {
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
        pns::lights::Action::End => {
            let _ = std::fs::remove_file(&marker);
        }
    }
}
/// End this session's wait on the operator directly: a state-only file move
/// in `clear_nag`'s style, with no event built, no config loaded and no
/// decision made.
///
/// TWO CALLERS NEED EXACTLY THIS, both in `hook_mode`: `prompt`, because the
/// operator answering a live wait by typing is not `resolved`'s signal
/// (PermissionRequest is decided off this hook's stdout, never off a later
/// PostToolBatch), and `resolved` itself, guarded there against a subagent's
/// batch. Ending is unconditional, unlike starting one: see
/// `update_blocked_marker`'s comment on why an End never checks the lamp
/// switches.
pub(crate) fn end_blocked_wait(session_id: &str) {
    if let Some(marker) = pns::lights::blocked_marker(&state_dir(), session_id) {
        let _ = std::fs::remove_file(&marker);
    }
}

#[cfg(test)]
#[path = "blocked_wait_markers/tests.rs"]
mod blocked_wait_markers_tests;
