use crate::JobSpool;

/// Keep the poll registered while the sensor is on, and cancelled while it is
/// not.
///
/// THE SETTINGS ARRIVE AS AN ARGUMENT rather than being read here, which is
/// what makes the sweep a function of the spool alone: the config read is the
/// caller's, and this can be driven a state directory at a time.
///
/// THE PENDING DUE IS KEPT, `schedule_lights_tick`'s rule for its own reason:
/// re-registering replaces the job by name, so a sweep that pushed `due` out
/// every thirty seconds would keep moving a five-second poll away from itself.
/// Only the LEASE is refreshed.
pub fn ensure_presence_poll(jobs: &impl JobSpool, interval: Option<u64>, now: u64) {
    let Some(interval) = interval else {
        // The failure is dropped for `record_decision`'s reason: a cancel that
        // did not land costs one more poll, and the lease ends it regardless.
        let _ = jobs.cancel(PRESENCE_JOB);
        return;
    };
    let pending = jobs.pending(PRESENCE_JOB).map(|job| job.due);
    // DUE NOW when nothing is pending, so the first sweep after the switch
    // goes on is followed by a reading on the next tick rather than one
    // interval later.
    let due = pending.filter(|due| *due > now).unwrap_or(now);
    let job = pns_domain::jobs::Job {
        id: PRESENCE_JOB.to_string(),
        due,
        until: due.max(now.saturating_add(PRESENCE_LEASE_SECS)),
        every: Some(interval),
        unless_marker: None,
        args: vec![
            "presence".to_string(),
            "poll".to_string(),
            PRESENCE_DAEMON_FLAG.to_string(),
        ],
    };
    // The failure is DROPPED here for `schedule_lights_tick`'s reason: a
    // registration that did not land must never cost the daemon a line a
    // second, and the next sweep tries again.
    let _ = jobs.schedule(&job, now);
}

/// The spool name the room sensor's poll is registered under.
const PRESENCE_JOB: &str = "presence";

/// How long that registration runs for. FIVE MINUTES, which is ten of the
/// daemon's own config reads at the production tick: long enough that a missed
/// sweep changes nothing, short enough that a daemon which stopped leaves
/// nothing polling the bridge behind it.
const PRESENCE_LEASE_SECS: u64 = 300;

/// The daemon's own spelling, passed by the registration in
/// `ensure_presence_poll` and by nothing else.
///
/// A FLAG RATHER THAN AN ENVIRONMENT VARIABLE, because the argv is what the
/// spool already records and what one parser already reads: an inherited
/// variable would also mark every unrelated process a poll ever started, and
/// the poll is the thing being described, not its ancestry.
pub const PRESENCE_DAEMON_FLAG: &str = "--daemon";
