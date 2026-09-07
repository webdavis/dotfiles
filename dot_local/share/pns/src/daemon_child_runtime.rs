use crate::*;

/// One child the daemon started, and the moment it stops being allowed to run.
pub(crate) struct Bounded {
    /// The job's own id, so `decide` can ask whether THIS job's child is
    /// still running rather than merely whether any child is.
    pub(crate) id: String,
    pub(crate) child: std::process::Child,
    pub(crate) expires_at: std::time::Instant,
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
pub(crate) fn spawn_job(job: &pns::daemon::Job) -> std::io::Result<std::process::Child> {
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
pub(crate) fn reap(children: &mut Vec<Bounded>) {
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

/// How many ticks a spawned job may run before it is killed, as a FLOOR.
///
/// THIRTY, so the bound moves with the tick and there is ONE knob rather than
/// two. In production that is thirty seconds, which is generous for the event
/// dispatch most of these children are: every channel inside one already
/// carries its own deadline, so a child still alive at this point is wedged
/// rather than slow. The LIGHTS tick is the exception, and `child_bound` is
/// where its own arithmetic lives.
const CHILD_TICKS: u32 = 30;

/// How long a spawned job may actually run before it is killed.
///
/// THE LIGHTS TICK IS THE ONE JOB WHOSE WORK IS AN INTERVAL, and it is named
/// here rather than generalised over every repeat. Every other child is an
/// event delivery whose channels each carry their own deadline, so one still
/// alive at `CHILD_TICKS` is wedged rather than slow and the tick-scaled bound
/// is exactly right for it. Widening the floor to all of them would only make a
/// wedged delivery take longer to kill.
///
/// THE TICK'S OWN ARITHMETIC, STATED: the longest interval it can be given
/// (`MAX_REFRESH_SECS`, thirty seconds), plus the longest a single write may
/// take at that interval (`tick_bridge_deadline`, a fifth of it, so six), plus
/// one reap tick, because a child is only noticed as gone on the pass after it
/// exits. Thirty-seven seconds at the production clock.
///
/// WHY IT IS NOT `CHILD_TICKS` ALONE: that made the tick's child life equal to
/// the longest interval a tick can be given, and a seamless breath issues its
/// last fade strictly INSIDE that interval and lets it finish after. At a
/// thirty-second refresh with 749ms spent resolving, the last write starts at
/// child time 29,999ms and its legal six-second reply was killed before the
/// tick could record where the lamp landed, leaving the next tick to resume
/// from a phase nothing had written. `max` keeps the tick-scaled bound wherever
/// it is the larger of the two, so a deliberately slow clock still gets the
/// generous child it always had.
pub(crate) fn child_bound(tick: Duration, id: &str) -> Duration {
    if id != LIGHTS_JOB {
        return tick * CHILD_TICKS;
    }
    let one_lights_tick = Duration::from_secs(pns::config::MAX_REFRESH_SECS)
        + tick_bridge_deadline(pns::config::MAX_REFRESH_SECS)
        + tick;
    (tick * CHILD_TICKS).max(one_lights_tick)
}

#[cfg(test)]
#[path = "daemon_child_runtime/tests.rs"]
mod daemon_child_runtime_tests;
