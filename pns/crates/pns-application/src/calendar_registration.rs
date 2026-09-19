use crate::{JobSpool, PollSetting};

/// Keep the calendar poll registered while the feature is on, and cancelled
/// while it is not.
///
/// `ensure_github_poll`'S SHAPE, AND FOR ITS REASONS: no event asks what the
/// calendar says, so nothing else would ever register this job, and one
/// registered at startup would die with its lease on the first daemon that
/// outran it. The settings arrive as an argument so the config read stays the
/// caller's, the PENDING due is kept so a sweep every thirty seconds does not
/// keep pushing the poll away from itself, and a failure is dropped because
/// the next sweep tries again.
///
/// THE POLL RUNS AS A CHILD, like every other job in the spool, which is what
/// keeps a calendar command that stalls out of the daemon's own loop: the
/// tick that started it goes on draining the spool, and the deadline the
/// command was given is what ends it.
pub fn ensure_calendar_poll(jobs: &impl JobSpool, setting: PollSetting, now: u64) {
    let interval = match setting {
        PollSetting::Every(interval) => interval,
        PollSetting::Unreadable => {
            crate::poll_lease::renew_lease(jobs, CALENDAR_JOB, CALENDAR_LEASE_SECS, now);
            return;
        }
        PollSetting::Off => {
            let _ = jobs.cancel(CALENDAR_JOB);
            return;
        }
    };
    let pending = jobs.pending(CALENDAR_JOB).map(|job| job.due);
    // DUE NOW when nothing is pending, so switching the feature on mutes a
    // meeting already under way rather than the next one.
    let due = pending.filter(|due| *due > now).unwrap_or(now);
    let job = pns_domain::jobs::Job {
        id: CALENDAR_JOB.to_string(),
        due,
        until: due.max(now.saturating_add(CALENDAR_LEASE_SECS)),
        every: Some(interval),
        unless_marker: None,
        args: vec!["quiet".to_string(), "calendar".to_string()],
    };
    let _ = jobs.schedule(&job, now);
}

/// The spool name the calendar poll is registered under.
const CALENDAR_JOB: &str = "quiet-calendar";

/// How long that registration runs for. FIFTEEN MINUTES, the GitHub poll's
/// own lease and for its reason: it outlasts several sweeps, so a missed one
/// changes nothing, and it is short enough that a daemon which stopped leaves
/// nothing reading the calendar behind it.
const CALENDAR_LEASE_SECS: u64 = 900;

#[cfg(test)]
mod tests;
