use super::RunDaemonTick;
use crate::{DaemonNotice, DaemonSpool, JobChildren, SpoolReading};

impl<S: DaemonSpool, C: JobChildren> RunDaemonTick<'_, S, C> {
    /// One CLAIMED record, re-read and acted on.
    ///
    /// THE RE-READ IS THE POINT. Between the peek that decided to act and the
    /// rename that took the record, a client can have replaced it with a refresh
    /// carrying a new due, a new lease and new arguments. Acting on the peek would
    /// fire the old argv and then delete the new record on the way out; acting on
    /// the claim fires whatever this daemon actually holds.
    pub(super) fn act(
        &mut self,
        claim: &S::Entry,
        id: &str,
        now: u64,
        notice: &mut impl FnMut(DaemonNotice),
    ) {
        match self.spool.read(claim, id) {
            // A RENAME MOVES A REGULAR FILE AS A REGULAR FILE, so this is not
            // reachable by the paths above; it is still answered rather than
            // ignored, because the alternative is a claim held forever.
            SpoolReading::Irregular => {
                notice(DaemonNotice::Output(format!(
                    "pns daemon: dropped `{id}`: it is not a regular file"
                )));
                self.release(claim, notice);
            }
            SpoolReading::Unusable(refusal) => {
                notice(DaemonNotice::Output(format!(
                    "pns daemon: dropped `{id}`: {refusal}"
                )));
                self.release(claim, notice);
            }
            SpoolReading::Job(job) => {
                // ASKED AGAIN, AND REDUNDANT WHILE THE PEEK ASKS IT TOO: the peek
                // stands a running job down before anything is claimed, so this is
                // only ever reached with no child of this id alive, and no test can
                // tell this argument from a literal `false`. It stays because the
                // peek is an optimisation over a re-read and this is the decision
                // the claim is actually acted on.
                let running = self.children.running(&job.id);
                match pns_domain::jobs::decide(&job, now, self.spool.marker_exists(&job), running) {
                    // The refresh this daemon claimed is not due yet, so it goes
                    // back CREATE-IF-ABSENT: a client that registered again in the
                    // meantime keeps its own record and this copy is dropped.
                    pns_domain::jobs::Verdict::Wait => match self.spool.hand_back(&job) {
                        Ok(_) => self.release(claim, notice),
                        Err(error) => {
                            notice(DaemonNotice::Error(format!(
                                "pns daemon: `{id}` could not be put back ({error})"
                            )));
                            self.release(claim, notice);
                        }
                    },
                    pns_domain::jobs::Verdict::Drop(reason) => {
                        notice(DaemonNotice::Output(format!(
                            "pns daemon: dropped `{id}` because {}",
                            reason.said()
                        )));
                        self.release(claim, notice);
                    }
                    pns_domain::jobs::Verdict::Fire => self.fire(&job, now, claim, notice),
                }
            }
        }
    }

    /// A working file this daemon is done with, removed and NAMED IF IT SURVIVES.
    ///
    /// A CLAIM THAT COULD NOT BE REMOVED IS A LEAK, not a nothing: it is invisible
    /// to the scan (the working prefix is outside the id charset), so it sits there
    /// until a hand removes it, and `claim` refuses to reuse a name already taken,
    /// which can wedge that one id after a pid is reused. One line naming the file
    /// is the whole remedy, and it costs nothing on the path where the remove
    /// works.
    fn release(&self, claim: &S::Entry, notice: &mut impl FnMut(DaemonNotice)) {
        if let Err(error) = self.spool.release(claim) {
            notice(DaemonNotice::Error(format!(
                "pns daemon: the working file {} could not be removed ({error}); it is left behind",
                self.spool.describe(claim)
            )));
        }
    }

    /// One claimed job re-armed and started, in that order.
    ///
    /// THE RE-ARM IS DURABLE BEFORE THE SPAWN. Written the other way round, a
    /// daemon killed between the two loses the repeat with the job already run,
    /// which is the lamp going dark on a loop that is still alive.
    ///
    /// AND THE RE-ARM IS CREATE-IF-ABSENT. A client that refreshed this id while
    /// the occurrence was claimed published the newer signal, and a rename here
    /// would overwrite it with the due and lease this daemon computed from the
    /// record it had already taken.
    fn fire(
        &mut self,
        job: &pns_domain::jobs::Job,
        now: u64,
        claim: &S::Entry,
        notice: &mut impl FnMut(DaemonNotice),
    ) {
        if let Some(next) = pns_domain::jobs::rearm(job, now) {
            match self.spool.hand_back(&next) {
                Ok(true) => {}
                Ok(false) => notice(DaemonNotice::Output(format!(
                    "pns daemon: `{}` was registered again while it ran, so its repeat stands down",
                    job.id
                ))),
                Err(error) => notice(DaemonNotice::Error(format!(
                    "pns daemon: `{}` will not repeat ({error})",
                    job.id
                ))),
            }
        }
        self.release(claim, notice);
        // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS NOT BEEN PERFORMED: a spawn
        // that failed is said out loud, because the alternative is a job that
        // reports as run and delivered nothing.
        //
        // AND A SPAWN THAT WORKED SAYS NOTHING, which is the daemon's own
        // no-chatter rule applied to the thing it actually does. The lights tick
        // repeats every twelve seconds for as long as its lease holds, so a line
        // per firing is 300 an hour in the file the log rotation then rotates a
        // real log out of. What a job has to say, the job says itself: its stderr
        // is the daemon's now.
        if let Err(error) = self.children.start(job) {
            notice(DaemonNotice::Error(format!(
                "pns daemon: `{}` could not start ({error})",
                job.id
            )));
        }
    }
}
