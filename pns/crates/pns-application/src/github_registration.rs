use crate::JobSpool;

/// Keep the GitHub poll registered while the source is on, and cancelled while
/// it is not.
///
/// `ensure_presence_poll`'S SHAPE, AND FOR ITS REASONS, which its own docblock
/// states word for word about this job too: no event asks for a notification
/// listing, so nothing else would ever register it, and a job registered once
/// at startup would die with its lease on the first daemon that outran it. The
/// settings arrive as an argument so the config read stays the caller's, the
/// PENDING due is kept because re-registering replaces the job by name and a
/// sweep that pushed `due` out every thirty seconds would keep moving the poll
/// away from itself, and every failure is dropped because the next sweep tries
/// again.
///
/// THE INTERVAL IS THE ONE THE SERVER LAST ASKED FOR, read off the poll's own
/// state by the caller rather than off the config: the documentation says to
/// obey `X-Poll-Interval`, and obeying it means the registration moves when
/// the header does.
pub fn ensure_github_poll(jobs: &impl JobSpool, interval: Option<u64>, now: u64) {
    let Some(interval) = interval else {
        let _ = jobs.cancel(GITHUB_JOB);
        return;
    };
    let pending = jobs.pending(GITHUB_JOB).map(|job| job.due);
    // DUE NOW when nothing is pending, so arming the source is followed by a
    // listing on the next tick rather than one interval later.
    let due = pending.filter(|due| *due > now).unwrap_or(now);
    let job = pns_domain::jobs::Job {
        id: GITHUB_JOB.to_string(),
        due,
        until: due.max(now.saturating_add(GITHUB_LEASE_SECS)),
        every: Some(interval),
        unless_marker: None,
        args: vec![
            "github".to_string(),
            "poll".to_string(),
            GITHUB_DAEMON_FLAG.to_string(),
        ],
    };
    let _ = jobs.schedule(&job, now);
}

/// The spool name the GitHub poll is registered under.
const GITHUB_JOB: &str = "github";

/// How long that registration runs for.
///
/// FIFTEEN MINUTES, which is the presence poll's five scaled to this job's own
/// interval: a lease has to outlast several sweeps so a missed one changes
/// nothing, and it has to be short enough that a daemon which stopped leaves
/// nothing polling GitHub behind it. At a 60-second interval that is fifteen
/// ticks of slack; at the 3600-second ceiling the lease expires between ticks
/// and the sweep re-registers it, which is the same self-renewal the presence
/// job relies on.
const GITHUB_LEASE_SECS: u64 = 900;

/// The daemon's own spelling, passed by the registration above and by nothing
/// else. A FLAG RATHER THAN AN ENVIRONMENT VARIABLE, for
/// `PRESENCE_DAEMON_FLAG`'s reason: the argv is what the spool already records
/// and what one parser already reads.
pub const GITHUB_DAEMON_FLAG: &str = "--daemon";

#[cfg(test)]
mod tests;
