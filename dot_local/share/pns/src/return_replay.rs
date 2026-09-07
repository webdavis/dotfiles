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
    if !pns::missed_notifications::should_replay(decision) {
        return;
    }
    // NOWHERE THE OPERATOR WOULD SEE IT IS NOT A REPLAY, and that is a
    // stronger test than "nowhere at all". MEASURED: an event narrowed with
    // `--remote-only`, and every event on a machine whose config enables only
    // a durable channel, claimed the queue, posted it into a log that already
    // holds all of it in full, and deleted it, with nothing the operator would
    // ever see. The empty plan (both narrowing flags, a typing mistake) is
    // refused by the same line, because nothing in an empty list is
    // decorative.
    //
    // WHICH LEGS DECORATE IS ROUTING'S ANSWER, carried out on the leg. Asking
    // it here by name, or by re-reading the declarations, would be the second
    // copy of a policy that then drifts, which is the mistake `run_event`
    // states about the mute a few lines above its own decision.
    if !decision.legs.iter().any(|leg| leg.decorative) {
        return;
    }
    // THE MOMENT IS CLAIMED BEFORE ANYTHING IS COUNTED, which is the whole
    // ownership rule and the reverse of what this used to do. Reading the
    // marker first and renaming it afterwards claims a DIFFERENT marker from
    // the one that was counted, because the winner republishes inside that
    // gap; MEASURED at roughly one run in thirty, two racers counted one loud
    // window and both posted it.
    //
    // THE CARD'S OWN SWITCH RIDES INTO THE CLAIM rather than returning in
    // front of it. Claiming the journal renames it out of the way, so a return
    // after that would consume the queue and deliver nothing, which is the one
    // outcome the four-way `Claimed` enum exists to prevent; handing the
    // switch in means the journal is never claimed at all when the card is off.
    let Moment::Owned { since, waiting } =
        claim_moment(decision.inputs.now_secs, recap.replay_card)
    else {
        // A RACER INSIDE SOMEBODY ELSE'S RETURN MOMENT SAYS NOTHING AT ALL.
        // The holder is about to deliver both halves, and this run has claimed
        // neither the window nor the queue, so there is nothing here to lose.
        return;
    };
    // THE WINDOW COMES OFF WHAT WAS CLAIMED, never off a second read: `since`
    // is the value that was renamed out from under every other racer, so a
    // racer holding a republished marker computes the empty window it deserves
    // rather than the one somebody else already posted.
    //
    // A MARKER AHEAD OF NOW IS NO WINDOW EITHER. A clock that moved backwards
    // is not a bracket, and the restore inside the claim kept the newer value,
    // so nothing is lost by refusing it here.
    let window = match (since, decision.inputs.now_secs) {
        (Some(since), Some(until)) if since <= until => Some((since, until)),
        _ => None,
    };
    let counted = window.map_or_else(Vec::new, |(since, until)| activity_in(since, until));
    // FOUR CLAUSES AND NONE OF THEM OPTIONAL. No window means no recap at all,
    // which is what stops a fresh install recapping the whole ring; the
    // threshold is what stops an ordinary afternoon becoming one; `digest` is
    // the operator's own switch over the Discord half; and a machine with no
    // durable route has nowhere for a recap to land, so the card must not
    // point at one.
    let fires =
        recap.digest && durable_route && window.is_some() && counted.len() >= recap.min_events;
    // THE DISCORD HALF GOES FIRST AND IN ITS OWN PROCESS, before the card, so
    // the card can say truthfully whether there is a recap to point at. The
    // spawn is a fork and an exec, so the card is dispatched microseconds
    // later; everything slow happens in the child.
    let posted = match window {
        Some((since, until)) if fires => spawn_recap(since, until),
        _ => false,
    };
    // THE TWO DELIVERIES ARE INDEPENDENT: an operator who wants the recap in
    // Discord and no card on the phone has asked for exactly that, which is
    // why this sits BELOW the spawn.
    if !recap.replay_card {
        return;
    }
    // TWO CARDS, ONE SITE, AND AT MOST ONE OF THEM. Over the threshold the
    // recap card is the delivery, whether or not anything was journaled, because
    // the window itself is the news; under it there is no recap, so an empty
    // queue is nothing to say and the catch-up card is unchanged.
    let detail = if fires {
        pns::missed_notifications::recap_card(
            &pns::missed_notifications::needing_you(&counted),
            counted.len(),
            waiting.len(),
            posted,
        )
    } else if waiting.is_empty() {
        return;
    } else {
        pns::missed_notifications::summary(&waiting)
    };
    // ONE SYNTHETIC EVENT, whatever the count. Empty project and branch,
    // because a batch spans both and `render::message` would otherwise prefix
    // the lot with one branch's name; empty channel, because an entry carries
    // none (the durable route already had the event); empty pane, which is the
    // call `doctor_mode` makes too, because a pane id from an hour ago may
    // name a pane that no longer exists. The title reads `pns · missed`, which
    // is visibly not a live agent card: a replayed card that looked live would
    // be lying about time.
    let replay = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "missed".to_string(),
        detail,
        ..Default::default()
    };
    // DISPATCHED DIRECTLY AND NEVER THROUGH `run_event`, which is the loop
    // this closes. A synthetic event fed back in would take a SECOND decision
    // (the second reading of one moment `GateInputs` exists to forbid), write
    // a second ring line for something that is not an event, fire a second
    // pulse, and RE-JOURNAL: under a mute the replay would journal itself and
    // the next one would replay the replay, forever, growing by one entry each
    // time. `doctor_mode` is the precedent in this file for the same split;
    // what is left after a decision has been taken is dispatch alone.
    //
    // THE LEGS ARE THIS DECISION'S OWN, verbatim. Deciding again would be a
    // second copy of routing's policy, which `routing` itself warns is how the
    // two come to drift. ACCEPTED CONSEQUENCE: the durable leg is among them,
    // so the summary is posted to a log that already holds every entry in it.
    // That is a duplicate in content and a new fact in kind.
    let _ = dispatch_legs(&decision.legs, false, &replay, home, mobile, hermes_key);
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
