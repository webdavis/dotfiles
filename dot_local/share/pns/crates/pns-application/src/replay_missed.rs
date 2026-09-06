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
use pns_domain::decision::Decision;
use pns_domain::missed::{self, Entry};
use pns_domain::notification::EventArgs;

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
pub struct ReplayMissedNotifications<'a> {
    pub moment: &'a dyn ReturnMoment,
    pub activity: &'a dyn ActivityRing,
    pub publisher: &'a dyn RecapPublisher,
    pub delivery: &'a dyn ReplayDelivery,
}

impl ReplayMissedNotifications<'_> {
    /// Deliver the catch-up for this event, or decline and say nothing.
    pub fn run(&self, decision: &Decision, recap: RecapPolicy, durable_route: bool) {
        if !missed::should_replay(decision) {
            return;
        }

        // NOWHERE THE OPERATOR WOULD SEE IT IS NOT A REPLAY, and that is a
        // stronger test than "nowhere at all". An event narrowed to a durable
        // channel alone would claim the queue, post it into a log that already
        // holds all of it, and delete it, with nothing the operator ever sees.
        if !decision.legs.iter().any(|leg| leg.decorative) {
            return;
        }

        // CLAIMED BEFORE ANYTHING IS READ. Two events arriving together must
        // not both replay the same window, and the claim is what decides which
        // one does.
        let Some(claim) = self
            .moment
            .claim(decision.inputs.now_secs, recap.replay_card)
        else {
            return;
        };

        let window = match (claim.since, decision.inputs.now_secs) {
            (Some(since), Some(until)) if since <= until => Some((since, until)),
            _ => None,
        };
        let counted: Vec<Entry> = window.map_or_else(Vec::new, |(since, until)| {
            self.activity.entries_between(since, until)
        });

        // THE DIGEST IS DURABLE AND THE CARD IS NOT, so the digest needs a
        // durable route to go to and a window worth the operator's attention;
        // the card below is raised on far weaker grounds.
        let fires =
            recap.digest && durable_route && window.is_some() && counted.len() >= recap.min_events;
        let posted = match window {
            Some((since, until)) if fires => self.publisher.publish(since, until),
            _ => false,
        };

        if !recap.replay_card {
            return;
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
            return;
        } else {
            missed::summary(&claim.waiting)
        };

        self.delivery.deliver(
            &EventArgs {
                agent: "pns".to_string(),
                state: "missed".to_string(),
                detail,
                ..EventArgs::default()
            },
            &decision.legs,
        );
    }
}

#[cfg(test)]
mod tests;
