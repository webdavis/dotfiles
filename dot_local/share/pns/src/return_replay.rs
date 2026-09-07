use crate::event_records::{ACTIVITY, ACTIVITY_KEPT, ACTIVITY_MAX_CHARS, ACTIVITY_READ_MAX};
use crate::*;

/// Put the journal in front of an operator who is here to see it, riding the
/// event that proved they are.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason: an
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and nothing here is worth a word to the operator anyway.
///
/// A LOSS ON A FAILED DELIVERY IS THE DESIGN, not an oversight. The engine's
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
    recap: &'a pns::config::Recap,
    home: &'a str,
    mobile: &'a Mobile,
    hermes_key: Option<String>,
    legs: &'a [pns::routing::Leg],
}

impl pns_application::ReturnMoment for CatchUp<'_> {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<pns_application::Claim> {
        match claim_moment(now, take_journal) {
            Moment::Owned { since, waiting } => Some(pns_application::Claim { since, waiting }),
            Moment::Busy => None,
        }
    }
}

impl pns_application::ActivityRing for CatchUp<'_> {
    fn record(&self, event: &pns::args::EventArgs, now: Option<u64>) {
        let _ = append_ring_line(
            &state_dir().join(ACTIVITY),
            &pns::missed_notifications::entry(event, now, ACTIVITY_MAX_CHARS),
            ACTIVITY_KEPT,
            ACTIVITY_READ_MAX,
        );
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<pns::missed_notifications::Entry> {
        activity_in(since, until)
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

/// Start the recap in a process of its own, and say whether it really started.
///
/// THE DIGEST NEVER RUNS IN THIS PROCESS. `run_event` is reached from
/// `pns hook prompt`, which the harness does NOT background, and from the
/// bashrc notifier, where a human is watching their prompt. Rendering and
/// posting a recap sits on neither. NEVER WAITED ON, so this process exits
/// exactly when it would have, and the child is reparented if it goes first.
///
/// AND IN A PROCESS GROUP OF ITS OWN, which is the other half of detachment
/// and used to be claimed rather than done. A hook the harness times out is
/// killed by GROUP, and so is a shell prompt taking `SIGINT`; a child left in
/// the parent's group goes with it, after the marker has already moved on, so
/// the window can never fire again and the card in the operator's hand points
/// at a recap nobody is writing.
///
/// `current_exe` RATHER THAN A PATH, so a test binary re-execs itself and a
/// moved install still works. ONLY THE TWO BOUNDS CROSS: the child re-reads the
/// ring itself, so nothing is serialized between them and nothing is lost if
/// the child never starts.
///
/// TWO INDEPENDENT READS OF ONE RING, STATED. The card's count is this
/// process's own read of the window and the recap's header is the child's, so
/// an event landing in the shared `until` second between them, or a prune, can
/// leave the two counts one apart. Each is honest about what IT read, which is
/// the same rule the header's own comment states about the ring's depth;
/// reconciling them would mean serializing a snapshot the child is deliberately
/// free to re-read.
///
/// THE ANSWER IS WHETHER A CHILD EXISTS, which is what the card says out loud.
/// A spawn that failed must never leave a card pointing at a recap nobody is
/// writing.
///
/// A CHILD THAT DIES COSTS ONE RECAP AND NOTHING ELSE, which is why nothing
/// supervises it: the activity ring is not consumed, the marker has already
/// moved, and the card already carried the counts.
fn spawn_recap(since: u64, until: u64) -> bool {
    let Ok(binary) = std::env::current_exe() else {
        return false;
    };
    let mut child = Command::new(binary);
    child
        .args(["recap", "--since", &since.to_string()])
        .args(["--until", &until.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // A NEW GROUP, WITH ITS OWN ID, which is what `setpgid(0, 0)` in the
        // forked child does and what the doc above promises.
        .process_group(0);
    // AN UNBOUNDED DEADLINE IS A TERMINAL'S CHOICE, NEVER A BACKGROUND
    // CHILD'S. `PNS_REMOTE_TIMEOUT=0` is curl's `-m 0`, no deadline at all,
    // which nobody is behind to interrupt here: a wedged gateway would keep
    // this process alive for good, and every later window would add another.
    if remote_deadline(std::env::var("PNS_REMOTE_TIMEOUT").ok().as_deref()).is_none() {
        child.env("PNS_REMOTE_TIMEOUT", RECAP_DEADLINE_SECS);
    }
    child.spawn().is_ok()
}
/// The deadline a detached recap falls back to when the environment asked for
/// none. Generous, because nobody is waiting on this process; finite, because
/// nobody is watching it either.
const RECAP_DEADLINE_SECS: &str = "30";
