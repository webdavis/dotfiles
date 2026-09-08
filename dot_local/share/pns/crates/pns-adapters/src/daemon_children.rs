use pns_application::JobChildren;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

pub struct DaemonChildren {
    children: Vec<Bounded>,
    tick: Duration,
}
impl DaemonChildren {
    pub fn new(tick: Duration) -> Self {
        Self {
            children: Vec::new(),
            tick,
        }
    }
}
impl JobChildren for DaemonChildren {
    fn reap(&mut self) {
        reap(&mut self.children);
    }
    fn running(&self, id: &str) -> bool {
        self.children.iter().any(|bounded| bounded.id == id)
    }
    fn start(&mut self, job: &pns_domain::jobs::Job) -> Result<(), String> {
        let child = spawn_job(job).map_err(|error| error.to_string())?;
        self.children.push(Bounded {
            id: job.id.clone(),
            child,
            expires_at: std::time::Instant::now() + child_bound(self.tick, &job.id),
        });
        Ok(())
    }
}

/// One child the daemon started, and the moment it stops being allowed to run.
pub(super) struct Bounded {
    /// The job's own id, so `decide` can ask whether THIS job's child is
    /// still running rather than merely whether any child is.
    pub(super) id: String,
    pub(super) child: std::process::Child,
    pub(super) expires_at: std::time::Instant,
}
/// The job's argv handed to THIS binary, detached.
///
/// `current_exe` AND NEVER A STORED PATH, exactly as `spawn_recap` does: the
/// record carries arguments, so nothing in the spool can name another program.
/// Anyone who can write a 0600 file in this directory can already run `pns`, so
/// this is a blast-radius limit rather than a security boundary, and it costs
/// nothing.
///
/// STDIN AND STDOUT NULL, STDERR INHERITED, and IN A GROUP OF ITS OWN, so
/// launchd stopping the daemon orphans a child in flight rather than killing it
/// mid-delivery.
///
/// STDERR IS THE ONE READER A JOB HAS. A job runs unattended with no terminal
/// behind it, so a complaint it writes goes wherever this puts that stream:
/// null sent it to `/dev/null`, and the lights tick's say-once memory then
/// recorded the complaint as SAID, so no later tick repeated it either. A lamp
/// renamed on the bridge was therefore reported exactly once, into nothing. The
/// daemon's plist points both of its own streams at `~/.local/log/`, so
/// inheriting is what puts a child's line in front of the operator.
///
/// STDOUT STAYS NULL, because that is where a job's ORDINARY output goes and
/// the ordinary case here is a tick that ran three times a minute and has
/// nothing to report. Only what could not be said anywhere else crosses.
pub(super) fn spawn_job(job: &pns_domain::jobs::Job) -> std::io::Result<std::process::Child> {
    let mut child = Command::new(std::env::current_exe()?);
    child
        .args(&job.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .process_group(0);
    child.spawn()
}

/// Every child looked at once, and any that outlived its bound killed.
///
/// `try_wait` AND NEVER `wait`. A blocking wait on a child that hangs holds the
/// whole loop, so one wedged delivery stops every later job: the clock would
/// pass every other test here and stop in production. The `wait` below runs
/// only on a child that has ALREADY been killed, which returns at once and is
/// what stops a zombie.
pub(super) fn reap(children: &mut Vec<Bounded>) {
    children.retain_mut(|bounded| match bounded.child.try_wait() {
        Ok(Some(_)) | Err(_) => false,
        Ok(None) if std::time::Instant::now() >= bounded.expires_at => {
            kill_group(bounded.child.id());
            // The direct child again, in case the group could not be signalled
            // at all, and then the wait that turns a killed child into a reaped
            // one rather than a zombie held for the daemon's lifetime.
            let _ = bounded.child.kill();
            let _ = bounded.child.wait();
            false
        }
        Ok(None) => true,
    });
}

/// Every process in a bounded child's group, killed.
///
/// THE GROUP AND NOT THE CHILD, which is the difference between a bound and a
/// bound that holds. `spawn_job` puts each job in a group of its own, and the
/// job is a `pns` that spawns a delivery of its own and waits on it: killing
/// the direct child alone leaves that delivery running, MEASURED still alive
/// 750ms past a 300ms bound, and a repeating job that hangs then accumulates
/// them. A negative pid names the group, which is the only reason
/// `process_group(0)` is set in the first place.
fn kill_group(pid: u32) {
    // NEVER 0 AND NEVER 1. `kill(0, ...)` signals THIS process's own group and
    // `kill(-1, ...)` signals every process the user owns, so a pid that is
    // neither a real child nor representable is refused rather than trusted.
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    if pid <= 1 {
        return;
    }
    // SAFE: `kill` takes two integers by value, reads and writes no memory this
    // process owns, and the only outcomes are a signal delivered or an errno
    // nothing here reads.
    unsafe { libc::kill(-pid, libc::SIGKILL) };
}

mod bounds;
use bounds::child_bound;

#[cfg(test)]
#[path = "daemon_children/tests.rs"]
mod daemon_children_tests;
