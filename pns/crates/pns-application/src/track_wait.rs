use crate::{JobSpool, SessionWaits};

/// This session's wait, recorded and timed: the row the escalation reads, and
/// the one leased job that wakes a fire an hour later.
///
/// ONE SEAM FOR BOTH, which is the whole design decision here. The row and the
/// job answer one question ("has this session been waiting, and since when"),
/// so they are written by the same call at the same moment as the blocked
/// marker; arming from the hook arms instead would put the two facts in
/// different places, where an event that reached one and not the other leaves
/// a row nothing will ever page about or a job with no row to find.
///
/// WHICH EVENTS START AND END A WAIT IS `blocked_marker_action` over
/// `pulse::LAMP_BLOCKED`, the list the lamps already carry, read rather than
/// copied: `plan-ready` and `asking` are waits by the same definition, and a
/// second list is a second thing to keep in step.
///
/// NO `unless_marker`. The nag's answered marker is written by every Stop and
/// StopFailure (`clear_nag`), so sharing it would cancel almost every
/// escalation before it fired; the row is the authority instead, and a fire
/// that finds a wait already over says so and pages nothing. What that costs
/// is one no-op spawn per answered block, an hour after it was answered.
///
/// EVERY FAILURE IS A LINE ON STDERR, never on stdout, and none of them
/// changes the exit code: this runs on a harness hook whose stdout the harness
/// parses (see `ArmNag`).
pub fn track_wait(
    waits: &impl SessionWaits,
    jobs: &impl JobSpool,
    session_id: &str,
    event_state: &str,
    window: u64,
    now: Option<u64>,
    mut warn: impl FnMut(&str),
) {
    let Some(session_id) = pns_domain::nag::usable(session_id) else {
        return;
    };
    if pns_domain::lights::phase::blocked_marker_action(event_state)
        == pns_domain::lights::phase::Action::End
    {
        // UNCONDITIONAL, unlike the start below: a row left behind by an
        // evening when the escalation was armed must still be cleared once the
        // operator switches the window off, or it would page the day they
        // switch it back on.
        if let Err(error) = end_wait(waits, session_id) {
            warn(&format!(
                "pns: state error (this session's wait could not be cleared: {error})"
            ));
        }
        return;
    }
    // A WINDOW OF ZERO IS THE FEATURE OFF, and the switch is read before the
    // clock so a machine that never armed this writes nothing at all.
    if window == WINDOW_OFF {
        return;
    }
    // NO CLOCK IS NO WAIT, never a wait at epoch zero: the elapsed time on the
    // page is measured against this number, and a zero would read as a block
    // that has stood since 1970.
    let (Some(now), Some(id)) = (now, pns_domain::stale::job_id(session_id)) else {
        return;
    };
    if let Err(error) = waits.begin(session_id, now) {
        warn(&format!(
            "pns: state error (this session's wait could not be recorded: {error}); \
             a stale block will not be escalated"
        ));
        return;
    }
    let due = now.saturating_add(window);
    let job = pns_domain::jobs::Job {
        id,
        due,
        // THE LEASE IS ONE MORE WINDOW PAST THE DUE SECOND, for `ArmNag`'s own
        // reason: a machine that slept through the window never spawns at all,
        // because an hour-old page about a block the operator has since seen
        // is history rather than news.
        until: due.saturating_add(window),
        every: None,
        unless_marker: None,
        // NO FREE TEXT REACHES THE SPOOL. The fire reads the row, so the
        // argv is the subcommand and nothing else; `pns stale` takes no
        // session argument for the same reason `pns nag` takes none.
        args: vec![pns_domain::stale::FIRE_WORD.to_string()],
    };
    if let Err(refusal) = jobs.schedule(&job, now) {
        warn(&format!(
            "pns: the stale-block escalation could not be scheduled ({refusal}); \
             this block will not be escalated"
        ));
    }
}

/// End this session's wait, with no event to judge and no job to arm.
///
/// THE TWO HOOK ARMS THAT END A WAIT DIRECTLY NEED EXACTLY THIS, for
/// `end_blocked_wait`'s reason one layer down: the operator answering by
/// typing is not `resolved`'s signal, and `resolved` itself is a tool batch
/// coming back. Neither builds an event, so neither has a state word for
/// `track_wait` to judge.
///
/// AN ID THIS ENGINE DOES NOT FOLLOW ENDS NOTHING, and that is success: it
/// named no row to begin with.
pub fn end_wait(waits: &impl SessionWaits, session_id: &str) -> Result<(), String> {
    match pns_domain::nag::usable(session_id) {
        Some(session_id) => waits.end(session_id),
        None => Ok(()),
    }
}

/// The window that means the escalation is off, in this crate's own spelling
/// of the config's default.
const WINDOW_OFF: u64 = 0;

#[cfg(test)]
mod tests;
