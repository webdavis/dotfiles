//! The nag: one card about every approval still waiting, at most one run at a
//! time.
//!
//! THE FIRE LOCK IS TAKEN FIRST AND GIVEN BACK LAST. Two runs nudging about
//! the same prompts is the failure this whole path exists inside, so nothing
//! below the claim happens without it.
//!
//! EVERY CLAIMED RECORD IS JUDGED HERE. The port claims and reports; `fate` is
//! the domain's and runs in this file, so a record dropped as answered and a
//! record dropped as unreadable are one decision with two reasons rather than
//! two adapters that happen to agree.

use crate::{NagRecords, RaiseNotification};
use pns_domain::EventArgs;
use pns_domain::nag::{self, Fate, Record};

/// What a nag run answered, as an exit code and what it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Another run holds the fire lock, so this one did nothing.
    Busy,
    /// Nothing is waiting.
    Nothing,
    /// One card was attempted, about this many waiting approvals.
    Nudged(usize),
}

/// The ports one nag run works over.
pub struct RunNag<'a, R, N> {
    pub records: &'a R,
    pub notifier: &'a N,
}

impl<R: NagRecords, N: RaiseNotification> RunNag<'_, R, N> {
    /// Nudge about whatever is still waiting.
    pub fn run(&self, now: u64, after_secs: u64, mut warn: impl FnMut(&str)) -> Outcome {
        if !self.records.claim_fire(now) {
            return Outcome::Busy;
        }

        let mut waiting: Vec<(String, Record)> = Vec::new();
        for claimed in self.records.claim_due(now) {
            match nag::fate(claimed.record.as_ref(), claimed.answered, now, after_secs) {
                Fate::Count => match claimed.record {
                    Some(record) => waiting.push((claimed.session_id, record)),
                    // `fate` answers `Count` only for a record it was given,
                    // so this arm is unreachable; dropping is the safe read of
                    // a record that somehow counted without parsing.
                    None => self
                        .records
                        .drop_claim(&claimed.session_id, Some(nag::Dropped::Unreadable)),
                },
                Fate::Drop(reason) => self.records.drop_claim(&claimed.session_id, Some(reason)),
            }
        }

        // THE OLDEST IS THE ONE THE CARD IS ABOUT, because the wait the
        // operator most needs to hear about is the one that has run longest.
        waiting.sort_by_key(|(_, record)| record.armed);
        let Some((_, oldest)) = waiting.first() else {
            self.records.release_fire();
            return Outcome::Nothing;
        };

        // MARKED BEFORE THE CARD, never after. A card raised against a session
        // nothing marked would nudge again on the next run, and the operator
        // would get two cards for one wait.
        for (session_id, _) in &waiting {
            if let Err(error) = self.records.mark_answered(session_id) {
                warn(&format!(
                    "pns nag: an answered marker could not be written ({error})"
                ));
            }
        }

        self.notifier.raise(&EventArgs {
            agent: oldest.agent.clone(),
            state: BLOCKED_STATE.to_string(),
            project: oldest.project.clone(),
            branch: oldest.branch.clone(),
            detail: nag::nudge(
                waiting.len(),
                now.saturating_sub(oldest.armed),
                &oldest.detail,
            ),
            pane: oldest.pane.clone(),
            ..EventArgs::default()
        });

        for (session_id, _) in &waiting {
            self.records.drop_claim(session_id, None);
        }
        self.records.release_fire();
        Outcome::Nudged(waiting.len())
    }
}

/// The state word a blocked approval and its nudge both carry.
const BLOCKED_STATE: &str = "blocked";

#[cfg(test)]
mod tests;
