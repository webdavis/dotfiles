use crate::*;

/// Put the journal in front of an operator who is here to see it, riding the
/// event that proved they are.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason: an
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and nothing here is worth a word to the operator anyway.
///
/// The journal remains owned until the existing delivery ledger accepts the
/// aggregate under its persisted replay identity. A queued replay is then the
/// ledger's responsibility, including failed or interrupted destination attempts.
/// A later return adopts that same identity and never rebuilds a queued card.
///
/// NOTHING IS PRINTED. The event path prints only what a reporting leg said,
/// and this rides an event whose stdout a hook reads.
///
/// `replay_card` IS THE OPERATOR'S SWITCH (`[recap] replay_card = false`) and
/// it gates THE CARD and nothing else. `record_missed` never learns the switch
/// exists, so the journal still records every miss and the doctor still counts
/// them: turning the card back on has something to deliver. `digest` is its
/// own switch over the Discord half, so card-only and recap-only are both
/// valid and neither implies the other.
///
/// THE ONE CARD SITE FOR BOTH FEATURES, which is the whole reason the recap
/// lives here rather than beside this. Two layers were locked, phone and
/// Discord, and a recap that raised its own phone card would put TWO cards on
/// the phone at one return moment. Worse, the case the recap exists for
/// journals NOTHING: a five-hour loop whose cards were all delivered and
/// forgotten leaves the queue empty, so the catch-up alone would raise no card
/// at all and the Discord recap would land with nothing pointing at it. So one
/// site composes at most one card, and which card it is depends on the window.
///
/// AND ONE CLAIM OVER BOTH, taken before anything is counted. `claim_moment`
/// arbitrates the whole return moment rather than the recap alone, so the two
/// halves cannot be won by two different racers; see its own comment for why
/// a claim per file MEASURED as two cards at one moment.
pub(crate) fn replay_missed(
    recap: pns_adapters::Recap,
    decision: &pns_domain::Decision,
    durable_route: bool,
    delivery: crate::delivery_runtime::DeliveryRuntime<'_>,
) {
    // THE DECISION IS THE USE CASE'S; the child spawn and the leg walk are
    // this side's. `[recap]`'s other fields (the summarizer, its deadline, the
    // repositories, the threading) never cross: they are the publisher's.
    let catch_up = CatchUp {
        moment: delivery.store,
        delivery,
    };
    pns_application::ReplayMissedNotifications { ports: &catch_up }.run(
        decision,
        pns_application::RecapPolicy {
            replay_card: recap.replay_card,
            digest: recap.digest,
            min_events: recap.min_events,
        },
        durable_route,
    );
}
/// THE COMPOSITION ROOT'S SIDE OF ONE CATCH-UP.
struct CatchUp<'a> {
    moment: &'a pns_adapters::SqliteStore,
    delivery: crate::delivery_runtime::DeliveryRuntime<'a>,
}

impl pns_application::ReturnMoment for CatchUp<'_> {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<pns_application::Claim> {
        pns_application::ReturnMoment::claim(self.moment, now, take_journal)
    }
    fn complete(&self) {
        pns_application::ReturnMoment::complete(self.moment);
    }
}

impl pns_application::ActivityRing for CatchUp<'_> {
    fn record(&self, event: &pns_domain::EventArgs, now: Option<u64>) {
        pns_application::ActivityRing::record(self.moment, event, now);
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<pns_domain::missed::Entry> {
        pns_application::ActivityRing::entries_between(self.moment, since, until)
    }
}

impl pns_application::RecapPublisher for CatchUp<'_> {
    fn publish(&self, since: u64, until: u64) -> bool {
        spawn_recap(since, until)
    }
}

impl pns_application::ReplayDelivery for CatchUp<'_> {
    fn deliver(
        &self,
        identity: &pns_application::SubmissionIdentity,
        event: &pns_domain::EventArgs,
        legs: &[pns_domain::routing::Leg],
    ) -> pns_application::ReplayHandoff {
        replay_handoff(self.delivery.submit(identity, event, legs, false, None))
    }
}

fn replay_handoff(
    result: Result<pns_application::Submitted, pns_application::LedgerFailure>,
) -> pns_application::ReplayHandoff {
    match result {
        Ok(pns_application::Submitted::Existing(_))
        | Ok(pns_application::Submitted::Attempted {
            sequence: Some(_), ..
        }) => pns_application::ReplayHandoff::Queued,
        _ => pns_application::ReplayHandoff::Retained,
    }
}

#[cfg(test)]
mod tests;
