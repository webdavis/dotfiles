use crate::JobSpool;
use pns_domain::lamps::LIGHTS_JOB;

/// How long the tick runs on after an ordinary event. A working loop emits
/// events constantly, so five minutes covers an agent's thinking gap without
/// covering a stall.
pub const ORDINARY_LEASE_SECS: u64 = 300;
/// And after a journalled one, which is an operator who is away or muted. The
/// glow has to survive the whole absence, and the absence is precisely when no
/// further event arrives to refresh this.
const JOURNALLED_LEASE_SECS: u64 = 12 * 60 * 60;
/// Register the repeating tick, or drop the refusal.
///
/// THE FAILURE IS DROPPED, exactly as `record_decision`'s is and for the same
/// reason: a lamp that did not re-arm must never cost a card, a line of stdout
/// or an exit code. `daemon::schedule` returns its error rather than printing
/// it precisely so each caller can state its own direction, and this one drops
/// it.
///
/// IT CANNOT BLOCK. The registration is one file written by rename into a
/// directory; there is no connection, no handshake and nothing to wait on, so
/// a daemon that is dead, wedged or mid-restart changes nothing about this
/// call.
///
/// TWO LEASE LENGTHS, off ONE question: was this event journalled. An ordinary
/// event means the operator is here and a working loop emits events
/// constantly, so five minutes covers an agent's thinking gap without covering
/// a stall. A journalled one means they are away or muted, which is exactly
/// when no further event will arrive to refresh this, and the glow has to
/// survive the whole absence.
///
/// THE DUE SECOND IS KEPT WHEN ONE IS ALREADY PENDING, and that is not
/// decoration: re-registering replaces the job by name, so an event storm that
/// pushed `due` out to `now + refresh` every time would keep moving the tick
/// away from itself and a busy machine's lamps would never be re-armed at all.
/// The lease is what every event refreshes; the schedule is left where the
/// last tick put it.
pub fn register_lights_tick(
    spool: &impl JobSpool,
    lights: Option<&pns_domain::lamps::config::Lights>,
    decision: &pns_domain::Decision,
    overrides: &pns_domain::Overrides,
) {
    // THE DECISION'S OWN CLOCK, like record_news and renew_loop_lease beside
    // this call: a fresh wall-clock read here would be a second reading of the
    // same moment, which is exactly the boundary R4-1 exists to close. NO
    // CLOCK IS NO REGISTRATION, never a job due at epoch zero.
    let (Some(lights), Some(now)) = (lights, decision.inputs.now_secs) else {
        return;
    };
    let lease = if pns_domain::missed::was_missed(decision, overrides) {
        JOURNALLED_LEASE_SECS
    } else {
        ORDINARY_LEASE_SECS
    };
    schedule_lights_tick(spool, lights, now, lease);
}
/// The tick registered to run for the next `lease_secs`, keeping whatever due
/// second is already pending.
///
/// THREE CALLERS AND ONE REGISTRATION, because the tick's lease is what decides
/// whether a lamp can EVER light, and three spellings of it would be three
/// answers. An event refreshes it, a lease taken by hand starts it, and the
/// tick renews its own while anything is still in flight.
///
/// THE DUE SECOND IS KEPT WHEN ONE IS ALREADY PENDING, and that is not
/// decoration: re-registering replaces the job by name, so an event storm that
/// pushed `due` out to `now + refresh` every time would keep moving the tick
/// away from itself and a busy machine's lamps would never be re-armed at all.
/// The lease is what every caller refreshes; the schedule is left where the
/// last tick put it.
pub fn schedule_lights_tick(
    spool: &impl JobSpool,
    lights: &pns_domain::lamps::config::Lights,
    now: u64,
    lease_secs: u64,
) {
    let pending = spool.pending(LIGHTS_JOB).map(|job| job.due);
    let due = pending
        .filter(|due| *due > now)
        .unwrap_or_else(|| now.saturating_add(lights.refresh_secs));
    let job = pns_domain::jobs::Job {
        id: LIGHTS_JOB.to_string(),
        due,
        // AT LEAST AS FAR AS THE DUE SECOND, because a lease that ended before
        // its own job's first run is a record `validate_shape` refuses, and a
        // refused registration is a lamp that never re-arms with nothing said
        // anywhere. It bites for any refresh interval longer than the ordinary
        // lease, even when the selected lease is shorter than the refresh interval.
        until: due.max(now.saturating_add(lease_secs)),
        every: Some(lights.refresh_secs),
        unless_marker: None,
        args: vec!["lights".to_string(), "tick".to_string()],
    };
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = spool.schedule(&job, now);
}

#[cfg(test)]
mod tests;
