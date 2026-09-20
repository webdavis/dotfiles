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
            expires_at: child_bound(self.tick, &job.id)
                .map(|bound| std::time::Instant::now() + bound),
        });
        Ok(())
    }
    fn terminate(&mut self, id: &str) {
        let Some(at) = self.children.iter().position(|bounded| bounded.id == id) else {
            return;
        };
        stop(self.children.remove(at));
    }
}

/// How long a terminated child is given to put itself down before it is
/// killed. Generous for a listener whose whole shutdown is closing a socket,
/// and short enough to sit well inside the twenty seconds launchd allows a job
/// after its own SIGTERM.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// How often the wait above looks, which is what decides how much of the grace
/// an ordinary exit actually spends.
const STOP_POLL: Duration = Duration::from_millis(20);

/// One child asked to stop, then made to.
///
/// SIGTERM AND THEN A BOUNDED WAIT, because the page closes its own listener
/// on the way out and a child killed outright would be the daemon deciding it
/// cannot. The group is signalled for `kill_group`'s reason: the direct child
/// alone leaves anything it spawned running.
fn stop(mut bounded: Bounded) {
    signal_group(bounded.child.id(), libc::SIGTERM);
    let deadline = std::time::Instant::now() + STOP_GRACE;
    loop {
        match bounded.child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if std::time::Instant::now() >= deadline => break,
            Ok(None) => std::thread::sleep(STOP_POLL),
        }
    }
    kill_group(bounded.child.id());
    let _ = bounded.child.kill();
    let _ = bounded.child.wait();
}

/// One child the daemon started, and the moment it stops being allowed to run.
pub(super) struct Bounded {
    /// The job's own id, so `decide` can ask whether THIS job's child is
    /// still running rather than merely whether any child is.
    pub(super) id: String,
    pub(super) child: std::process::Child,
    /// When this child stops being allowed to run, or `None` for one whose
    /// work is to stay up.
    pub(super) expires_at: Option<std::time::Instant>,
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
        Ok(None) if expired(bounded) => {
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

/// Whether a child has outlived its bound. One with no bound never has.
fn expired(bounded: &Bounded) -> bool {
    bounded
        .expires_at
        .is_some_and(|at| std::time::Instant::now() >= at)
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
    signal_group(pid, libc::SIGKILL);
}

/// One signal to every process in a child's group.
fn signal_group(pid: u32, signal: libc::c_int) {
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
    unsafe { libc::kill(-pid, signal) };
}

mod bounds;
use bounds::child_bound;

#[cfg(test)]
#[path = "daemon_children/tests.rs"]
mod daemon_children_tests;
