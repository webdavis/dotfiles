use crate::{DaemonNotice, DaemonSpool, JobChildren, SpoolReading};
use std::collections::BTreeSet;

use super::RunDaemonTick;

/// One pass over the spool, under a protocol with THREE INVARIANTS.
///
/// 1. **A CLIENT ALWAYS WINS.** Every write this daemon makes into the spool
///    (a re-arm, a put-back) is create-if-absent, so a registration or a
///    refresh that landed while a record was claimed keeps its name and the
///    daemon's older copy is discarded. An overwriting rename here would put a
///    stale due, lease and argv back over the newest signal, which is the one
///    guarantee the id-is-the-filename refresh rule makes.
/// 2. **THE DAEMON ACTS ONLY ON WHAT IT OWNS.** A read-only peek decides one
///    thing and one only: whether there is nothing to do. Everything else
///    claims the entry by rename FIRST and re-reads the claim, so the record
///    that fires is the record this daemon took, never one a refresh replaced
///    between the look and the act. A `Wait` is never claimed, because a wait
///    performs no action and renaming a waiting job out and back would be the
///    very write invariant 1 forbids.
/// 3. **ONE OCCURRENCE RUNS ONCE.** The rename is still the arbiter and it is
///    now taken before the content is read, so of two daemons exactly one
///    holds the record and the loser reads nothing at all.
///
/// THE RESIDUAL WINDOWS, STATED HONESTLY. A refresh that lands AFTER the claim
/// is taken cannot stop the occurrence already claimed from running, so the
/// operator can see one card from the record that was in flight plus the
/// refreshed job afterwards. Nothing is LOST and nothing runs twice; the old
/// occurrence simply ran. A refresh that lands after the claim also wins the
/// re-arm's link, so the repeat continues on the client's terms rather than the
/// daemon's. And a claim this process took and could not remove holds its own
/// working name; the line naming it is printed either way, because a job that
/// vanished with nothing in the log is the failure that costs the most to find.
impl<S: DaemonSpool, C: JobChildren> RunDaemonTick<'_, S, C> {
    pub(super) fn drain(
        &mut self,
        now: u64,
        reported: &mut BTreeSet<S::Entry>,
        notice: &mut impl FnMut(DaemonNotice),
    ) {
        for entry in self.spool.entries() {
            let Some(id) = self.spool.id(&entry) else {
                continue;
            };
            match self.spool.read(&entry, &id) {
                // SAID ONCE, never once a tick: the file is left where it is, so
                // the alternative is one line a second about a thing nobody is
                // going to fix while the daemon is watching.
                SpoolReading::Irregular => {
                    if reported.insert(entry.clone()) {
                        notice(DaemonNotice::Error(format!(
                            "pns daemon: {} is not a regular file; left alone and never opened",
                            self.spool.describe(&entry)
                        )));
                    }
                }
                // NOTHING TO DO, DECIDED WITHOUT TOUCHING IT. This is the only
                // verdict a peek is allowed to be the last word on.
                SpoolReading::Job(job)
                    if pns_domain::jobs::decide(
                        &job,
                        now,
                        self.spool.marker_exists(&job),
                        self.children.running(&job.id),
                    ) == pns_domain::jobs::Verdict::Wait => {}
                // Anything else is an ACTION, so the record is taken first and read
                // again afterwards. A failed claim means another run got there,
                // which is exactly what the rename is for.
                _ => {
                    if let Some(claim) = self.spool.claim(&entry) {
                        self.act(&claim, &id, now, notice);
                    }
                }
            }
        }
    }
}
