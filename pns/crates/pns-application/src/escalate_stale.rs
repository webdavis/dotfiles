//! The fire: one page about each session that has been blocked past the
//! window, and only where the operator could act on it.
//!
//! THE ORDER IS THE BEHAVIOR, which is why this is a use case and not a
//! sequence of calls at the composition root. The rows are read before the
//! gate so a suppressed fire can SAY how many waits it held back rather than
//! going quiet; the gate is read before any claim so a suppression leaves
//! `escalated_at` unset and a later fire can still escalate the block; and the
//! claim is taken before the page so a page that is attempted is never
//! attempted twice.

use crate::{RaiseNotification, StaleWaits};
use pns_domain::SurfaceReading;
use pns_domain::stale::{self, Gate, Suppressed};

/// What one fire did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleOutcome {
    /// The window is zero, so the escalation is off.
    Off,
    /// Nothing has been waiting long enough.
    Nothing,
    /// This many waits were found and none paged, because the operator could
    /// not act on one.
    Held { waiting: usize, why: Suppressed },
    /// This many pages were attempted, one per stuck session.
    Paged(usize),
}

/// The ports one fire works over.
pub struct EscalateStaleBlocks<'a, W, N> {
    pub waits: &'a W,
    pub notifier: &'a N,
}

impl<W: StaleWaits, N: RaiseNotification> EscalateStaleBlocks<'_, W, N> {
    pub fn run(&self, now: u64, window: u64, reading: &SurfaceReading) -> StaleOutcome {
        // A CONFIG THAT TURNED THE FEATURE OFF BETWEEN ARMING AND FIRING MEANS
        // NO PAGE, which is `nag_mode`'s own reading: the operator cancelled
        // the timer, and a page from it would be the feature ignoring them.
        // The rows are left alone, because the session's own next event clears
        // them (every Stop does) rather than accumulating.
        if window == WINDOW_OFF {
            return StaleOutcome::Off;
        }
        let waiting = self.waits.waiting_since(now.saturating_sub(window));
        if waiting.is_empty() {
            return StaleOutcome::Nothing;
        }
        // BEFORE ANY CLAIM. A suppressed fire must leave every row exactly as
        // it found it, so a later fire can still escalate the block.
        //
        // WHICH LATER FIRE, IF ANY, IS NOT PROMISED HERE. The job `track_wait`
        // arms is a one-shot, the shape `arm_nag` already uses and for its
        // reason (a held-back nag is lost rather than queued), so what reaches
        // a suppressed row is the session's next wait-starting event, another
        // session's fire sweeping globally, or `pns stale` typed at the desk.
        if let Gate::Skip(why) = stale::gate(
            reading.surface,
            reading.screen_locked,
            reading.desk_input_age,
            window,
        ) {
            return StaleOutcome::Held {
                waiting: waiting.len(),
                why,
            };
        }
        let mut paged = 0;
        for blocked in &waiting {
            // ONE ROW, ONE CLAIM, and a row another fire already claimed is
            // skipped rather than paged a second time.
            if !self.waits.claim(&blocked.session, now) {
                continue;
            }
            self.notifier.raise(&stale::page(blocked, now));
            paged += 1;
        }
        match paged {
            0 => StaleOutcome::Nothing,
            paged => StaleOutcome::Paged(paged),
        }
    }
}

/// The window that means the escalation is off.
///
/// THE FIRE OWNS IT and `track_wait` reads this one, so arming and firing
/// cannot disagree about what switches the feature off. The other two
/// spellings are each a different layer's own (`config`'s accepted value and
/// the composition root's fallback for a config it could not read), which is
/// the shape `NAG_OFF` already has.
pub(crate) const WINDOW_OFF: u64 = 0;

#[cfg(test)]
mod tests;
