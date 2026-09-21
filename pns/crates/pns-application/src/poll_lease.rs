use crate::JobSpool;

/// Renew the lease on a poll that is already registered, leaving its due and
/// its interval where they are. Nothing pending means nothing to renew.
///
/// THIS IS THE UNREADABLE-CONFIG PATH for every one of the daemon's own polls:
/// the sweep learned nothing, so the last registration stands and only its
/// lease moves. The failure is dropped for the registrations' own reason: the
/// next sweep tries again, and the lease ends the job if none succeeds.
pub(crate) fn renew_lease(jobs: &impl JobSpool, id: &str, lease_secs: u64, now: u64) {
    let Some(pending) = jobs.pending(id) else {
        return;
    };
    let job = pns_domain::jobs::Job {
        until: pending.until.max(now.saturating_add(lease_secs)),
        ..pending
    };
    let _ = jobs.schedule(&job, now);
}
