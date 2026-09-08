use super::{HeldLock, claim_lock};
use pns_application::LampTickClaim;
use std::path::PathBuf;

pub struct FileLampTick(pub PathBuf);
impl LampTickClaim for FileLampTick {
    type Guard = HeldLock;
    fn claim(&self, now_secs: u64) -> Option<Self::Guard> {
        let lock = self.0.join(LIGHTS_TICK_LOCK);
        claim_lock(&lock, now_secs, lights_tick_stale_secs()).then_some(HeldLock(lock))
    }
}

/// Where a lights tick holds the whole house for as long as it is driving it.
///
/// THE DAEMON'S OWN BOOKKEEPING IS NOT A LOCK. `decide` refuses to fire a
/// second lights child while the first is still listed, and that list is ONE
/// process's memory: a tick the operator ran by hand and an orphan left behind
/// by a daemon replacement are both invisible to it. Two ticks driving one lamp
/// interleave their fades against two schedules, and the phase the LAST of them
/// writes is the one the next tick resumes off, so the breath it picks up is
/// one no lamp was ever running. A file the operating system arbitrates is the
/// only guard every writer can see.
///
/// IT DOES NOT LOCK OUT THE EVENT PATH, deliberately. The operator's return
/// clears the held record from a process that holds no lock and must never wait
/// on one; `run_tick_writes` re-reads the record instead and stands down when
/// it moved, which is the guard that case has always had.
const LIGHTS_TICK_LOCK: &str = "lights-tick.lock";

/// How long a lights tick's lock is believed before it is read as an orphan.
///
/// `child_bound`'S OWN ARITHMETIC FOR THIS JOB, because it bounds the same
/// process: the longest interval the config permits, plus the longest a single
/// write may take at that interval, plus the second the daemon takes to notice
/// the child is gone. A tick still holding the lock past that has already been
/// killed, so the file is leavings. Standing down for a live holder costs one
/// interval of an unchanged lamp; stealing the lock from one that is still
/// driving is the failure the lock exists to stop, so the bound errs long.
fn lights_tick_stale_secs() -> u64 {
    crate::MAX_REFRESH_SECS
        + pns_domain::lamps::tick_bridge_deadline(crate::MAX_REFRESH_SECS).as_secs()
        + 1
}
