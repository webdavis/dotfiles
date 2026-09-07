use crate::Bridge;
use pns_application::{PollClaim, PresencePoll};
use pns_domain::RawPresence;
use std::path::Path;

pub struct BridgePresencePoll<'a, B> {
    pub bridge: &'a B,
    pub state: &'a Path,
}

impl<B: Bridge> PresencePoll for BridgePresencePoll<'_, B> {
    type Guard = std::fs::File;
    fn claim(&self) -> PollClaim<Self::Guard> {
        // THE DIRECTORY IS MADE BEFORE THE LOCK, because the lock now runs ahead
        // of `publish_state_line`, which used to be the thing that made it: a
        // first poll on a machine with no state directory yet would otherwise fail
        // to take a lock nobody holds and never publish at all.
        let _ = std::fs::create_dir_all(self.state);
        // HELD BY THE HANDLE AND NOT BY THE NAME, which is what makes a killed
        // poller cost nothing: the kernel closes its file and the lock is gone.
        match super::lock::claim(&self.state.join(super::lock::LOCK_FILE)) {
            super::lock::Claim::Held(file) => PollClaim::Held(file),
            super::lock::Claim::Busy => PollClaim::Busy,
            super::lock::Claim::Unavailable => PollClaim::Unavailable,
        }
    }
    fn read(&self, watched: &[String], now: u64) -> Option<RawPresence> {
        super::bridge::poll(self.bridge, watched, now)
    }
    fn publish(&self, reading: &RawPresence) -> bool {
        // A READING THIS FORMAT CANNOT CARRY PUBLISHES NOTHING, the same direction
        // a silent bridge takes. `render` used to substitute the poll-only line
        // for one, which says "the bridge answered and no watched room reported"
        // on evidence that said a watched room had.
        let Some(line) = super::state_file::render(reading) else {
            return false;
        };
        // FAIL-QUIET, in `remember_staleness`'s style: an unwritable state
        // directory costs the reading, which ages out on its own.
        crate::publish_state_line(&self.state.join(super::state_file::STATE_FILE), &line).is_ok()
    }
}
