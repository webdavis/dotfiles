use crate::{DaemonNotice, DaemonSpool, JobChildren};
use std::collections::BTreeSet;

/// Everything one turn of the daemon's loop does, in the ONE ORDER that makes
/// `decide`'s running answer true.
///
/// REAPED BEFORE THE SPOOL IS DRAINED, so a child `decide` finds still in
/// `children` really is alive THIS pass. Reaped the other way round, a child
/// that exited moments ago still reads as running and holds its own due
/// occurrence to one more `Wait` than it needed, which on the lights job is a
/// tick of a lamp that has stopped breathing.
///
/// IT IS A FUNCTION AND NOT FOUR LINES IN THE LOOP for exactly that reason:
/// the order is the behaviour, so a test has to be able to run it in the
/// order production runs it rather than in one of its own.
///
/// A SECOND THAT COULD NOT BE READ STOPS THE DRAIN AND NEVER THE REAP. A bound
/// is still a bound with no wall clock to publish against, and a child left
/// running past its own because the clock would not answer is the one failure
/// here that accumulates.
pub struct RunDaemonTick<'a, S, C> {
    pub spool: &'a S,
    pub children: &'a mut C,
}

impl<S: DaemonSpool, C: JobChildren> RunDaemonTick<'_, S, C> {
    pub fn run(
        &mut self,
        now: Option<u64>,
        reported: &mut BTreeSet<S::Entry>,
        mut notice: impl FnMut(DaemonNotice),
    ) {
        self.children.reap();
        let Some(now) = now else {
            return;
        };
        // FAIL-QUIET: a heartbeat that did not land costs one doctor line.
        self.spool.heartbeat(now);
        self.drain(now, reported, &mut notice);
    }
}

mod claimed;
mod drain;

#[cfg(test)]
mod tests;
