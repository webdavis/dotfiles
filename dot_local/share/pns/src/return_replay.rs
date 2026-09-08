use crate::*;

/// Put the journal in front of an operator who is here to see it, riding the
/// event that proved they are.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason: an
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and nothing here is worth a word to the operator anyway.
///
/// A completed failed attempt still consumes its batch. A crash or unwind
/// before completion leaves its claimed rows for later adoption. This keeps the
/// legacy fire-and-forget policy without deleting before the attempt runs.
/// The accepted failure policy remains: The engine's
/// contract is fire-and-forget for every producer; every journaled event
/// already reached the durable log in full, so nothing is lost that a human
/// cannot recover; re-journaling against a wedged channel is an unbounded
/// retry that grows the file every event; and `dispatch_legs`' outcomes cannot
/// tell delivery from perception in any case, because an executable channel
/// that ran answers `Silent` by design.
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
    recap: pns::config::Recap,
    decision: &pns::engine::Decision,
    home: &str,
    mobile: &Mobile,
    hermes_key: Option<String>,
    durable_route: bool,
) {
    // THE DECISION IS THE USE CASE'S; the child spawn and the leg walk are
    // this side's. `[recap]`'s other fields (the summarizer, its deadline, the
    // repositories, the threading) never cross: they are the publisher's.
    let catch_up = CatchUp {
        moment: pns_adapters::SqliteStore::for_records(state_dir()),
        recap: &recap,
        home,
        mobile,
        hermes_key,
        legs: &decision.legs,
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
    moment: pns_adapters::SqliteStore,
    recap: &'a pns::config::Recap,
    home: &'a str,
    mobile: &'a Mobile,
    hermes_key: Option<String>,
    legs: &'a [pns::routing::Leg],
}

impl pns_application::ReturnMoment for CatchUp<'_> {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<pns_application::Claim> {
        pns_application::ReturnMoment::claim(&self.moment, now, take_journal)
    }
    fn complete(&self) {
        pns_application::ReturnMoment::complete(&self.moment);
    }
}

impl pns_application::ActivityRing for CatchUp<'_> {
    fn record(&self, event: &pns::args::EventArgs, now: Option<u64>) {
        pns_application::ActivityRing::record(&self.moment, event, now);
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<pns_domain::missed::Entry> {
        pns_application::ActivityRing::entries_between(&self.moment, since, until)
    }
}

impl pns_application::RecapPublisher for CatchUp<'_> {
    fn publish(&self, since: u64, until: u64) -> bool {
        let _ = self.recap;
        spawn_recap(since, until)
    }
}

impl pns_application::ReplayDelivery for CatchUp<'_> {
    fn deliver(&self, event: &pns::args::EventArgs, legs: &[pns::routing::Leg]) {
        let _ = self.legs;
        let _ = dispatch_legs(
            legs,
            false,
            event,
            self.home,
            self.mobile,
            self.hermes_key.clone(),
        );
    }
}
