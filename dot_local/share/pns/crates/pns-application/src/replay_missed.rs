//! The catch-up: what the operator missed while they were away, delivered once
//! when they come back.
//!
//! IT IS A DECISION AND NOT AN ORDERING, unlike the record tail beside it.
//! Four refusals come first, each for its own reason, and only then is there a
//! window to count, a digest to weigh and a card to compose.
//!
//! ONE RETURN IS ONE CATCH-UP. The moment is claimed before anything is read,
//! so two events arriving together cannot both replay the same window.

use crate::ports::delivery::{RecapPublisher, ReplayDelivery};
use crate::ports::records::{ActivityRing, ReturnMoment};
use pns_domain::Decision;
use pns_domain::EventArgs;
use pns_domain::missed::{self, Entry};

/// The operator's `[recap]` answers, as this decision needs them.
///
/// THREE FIELDS AND NOT THE WHOLE TABLE. The summarizer, its deadline, the
/// repositories and the threading are the publisher's business; these three
/// are what decide whether anything is delivered at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecapPolicy {
    /// Whether a card is raised for the operator at all.
    pub replay_card: bool,
    /// Whether a durable digest is published beside it.
    pub digest: bool,
    /// How many events a window must hold before a digest is worth publishing.
    pub min_events: usize,
}

/// The ports one catch-up runs over.
pub struct ReplayMissedNotifications<'a, P> {
    pub ports: &'a P,
}

impl<P> ReplayMissedNotifications<'_, P>
where
    P: ReturnMoment + ActivityRing + RecapPublisher + ReplayDelivery,
{
    /// Deliver the catch-up for this event, or decline and say nothing.
    pub fn run(
        &self,
        decision: &Decision,
        recap: RecapPolicy,
        durable_route: bool,
    ) -> Option<crate::ReplayHandoff> {
        if !missed::should_replay(decision) {
            return None;
        }

        // NOWHERE THE OPERATOR WOULD SEE IT IS NOT A REPLAY, and that is a
        // stronger test than "nowhere at all". An event narrowed to a durable
        // channel alone would claim the queue, post it into a log that already
        // holds all of it, and delete it, with nothing the operator ever sees.
        if !decision.legs.iter().any(|leg| leg.decorative) {
            return None;
        }

        // CLAIMED BEFORE ANYTHING IS READ. Two events arriving together must
        // not both replay the same window, and the claim is what decides which
        // one does.
        let claim = ReturnMoment::claim(self.ports, decision.inputs.now_secs, recap.replay_card)?;

        if claim
            .replay
            .as_ref()
            .is_some_and(|batch| batch.state == crate::ReplayState::Queued)
        {
            ReturnMoment::complete(self.ports);
            return Some(crate::ReplayHandoff::Queued);
        }
        let until = claim
            .replay
            .as_ref()
            .map_or(decision.inputs.now_secs, |batch| batch.until);
        let window = match (claim.since, until) {
            (Some(since), Some(until)) if since <= until => Some((since, until)),
            _ => None,
        };
        let counted: Vec<Entry> = window.map_or_else(Vec::new, |(since, until)| {
            ActivityRing::entries_between(self.ports, since, until)
        });

        // THE DIGEST IS DURABLE AND THE CARD IS NOT, so the digest needs a
        // durable route to go to and a window worth the operator's attention;
        // the card below is raised on far weaker grounds.
        let fires =
            recap.digest && durable_route && window.is_some() && counted.len() >= recap.min_events;
        let posted = match window {
            Some((since, until)) if fires => RecapPublisher::publish(self.ports, since, until),
            _ => false,
        };

        if !recap.replay_card {
            ReturnMoment::complete(self.ports);
            return None;
        }
        // A CARD WITH NOTHING IN IT IS NOISE. With no digest to point at and
        // nothing waiting, there is no sentence to write.
        let detail = if fires {
            missed::recap_card(
                &missed::needing_you(&counted),
                counted.len(),
                claim.waiting.len(),
                posted,
            )
        } else if claim.waiting.is_empty() {
            ReturnMoment::complete(self.ports);
            return None;
        } else {
            missed::summary(&claim.waiting)
        };

        let Some(batch) = claim.replay else {
            return Some(crate::ReplayHandoff::Retained);
        };
        let handoff = ReplayDelivery::deliver(
            self.ports,
            &batch.identity,
            &EventArgs {
                agent: "pns".to_string(),
                state: "missed".to_string(),
                detail,
                ..EventArgs::default()
            },
            &decision.legs,
        );
        if handoff == crate::ReplayHandoff::Queued {
            ReturnMoment::complete(self.ports);
        }
        Some(handoff)
    }
}

#[cfg(test)]
mod tests;
