use crate::JobSpool;

/// How long past its due second an unstated lease runs. A minute: long enough
/// that a busy tick or a slow boot still delivers, short enough that a machine
/// asleep through the moment wakes to a job whose point has passed.
const DEFAULT_LEASE_SLACK_SECS: u64 = 60;

/// `--until` in its two spellings.
pub enum Until {
    Epoch(u64),
    FromNow(u64),
}

/// Everything `schedule` was asked for, before a clock is read.
pub struct ScheduleJob {
    pub id: String,
    pub in_secs: u64,
    pub every: Option<u64>,
    pub until: Option<Until>,
    pub marker: Option<String>,
    pub args: Vec<String>,
}

impl ScheduleJob {
    pub fn run(&self, spool: &impl JobSpool, now: Option<u64>) -> Result<(), String> {
        let now = now.ok_or("this machine has no clock to schedule against")?;
        let due = now.saturating_add(self.in_secs);
        let job = pns_domain::jobs::Job {
            id: self.id.clone(),
            due,
            until: match self.until {
                Some(Until::Epoch(epoch)) => epoch,
                Some(Until::FromNow(seconds)) => now.saturating_add(seconds),
                // A LEASE IS NEVER ABSENT, only unstated: a job with no expiry is
                // the parked job the whole design refuses, so an unstated one gets
                // a small slack past its due second.
                None => due.saturating_add(DEFAULT_LEASE_SLACK_SECS),
            },
            every: self.every,
            unless_marker: self.marker.clone(),
            args: self.args.clone(),
        };
        spool.schedule(&job, now)
    }
}

pub fn cancel_job(spool: &impl JobSpool, id: &str) -> Result<bool, String> {
    spool.cancel(id)
}
