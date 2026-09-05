//! The journal of notifications the operator could not have perceived: the
//! JSON codec that writes and reads one entry, and the three predicates that
//! decide whether an event was missed.
//!
//! POLICY ONLY, in `decision_log`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment, no
//! file and no printing. The composition root reads the world, decides where
//! the file is, appends what comes back and prints what the doctor asks for.
//! This module never learns where the journal lives.
//!
//! WHY THIS IS NOT THE DECISION RING. The two files have different readers.
//! The ring is read by a human through `pns doctor` and therefore admits no
//! free text at all; the journal is read by the replayer and is useless
//! without the event's own text. Fusing them would mean either printing
//! content to a terminal or journaling nothing worth replaying.
//!
//! What a miss COMPOSES INTO moved to `pns-domain`. The codec stays because
//! this crate is where `serde_json` lives, and the three predicates stay
//! because they answer over the engine's `Decision`.

use crate::args::EventArgs;
use crate::engine::{Decision, Overrides};
use crate::surface::{Surface, Visibility};

pub use pns_domain::missed::{
    Entry, KEPT, NEEDS_YOU, event_count, needing_you, recap_card, summary, waiting_line,
};

/// Whether the operator COULD NOT HAVE PERCEIVED this event.
///
/// Three clauses, all over values the record site already holds and none of
/// them a second reading. The plan said nothing, nobody was looking at the
/// origin pane, and the card was not skipped because another route already
/// carried it.
///
/// THE SURFACE HALF OF THE WATCHING CLAUSE is what saves the Away row: an
/// away operator is watching nothing, and a desk display showing the origin
/// pane to an empty chair is exactly the reading that must not suppress.
/// `surface::plan` reads `watching` the same way (it consults visibility only
/// in the Desk and Mobile arms), so this is that rule restated over the same
/// two values rather than a second rule.
///
/// `plan.pulse` IS DELIBERATELY NOT READ: the lights are decoration, and the
/// quiet window suppresses only them.
///
/// IT IS PLAN-LEVEL AND NOT DELIVERY-LEVEL, and that is a decision. A card
/// the plan called for and a channel failed to deliver is a truer miss than a
/// muted one, and it is still out of scope: `routing` derives legs FROM the
/// plan and drops the decoration on the way, so asking "did the leg carrying
/// the card fail" means re-deriving that policy here, which is the second
/// copy of a rule that then drifts. Two limits follow and are named rather
/// than left to be discovered. An event narrowed with both `--local-only` and
/// `--remote-only` reaches no channel while its plan still says banner, so it
/// is not journaled (it prints its own line and the decision log records it).
/// An event whose plan called for a card on a machine with no phone channel
/// configured is not journaled either.
pub fn was_missed(decision: &Decision, overrides: &Overrides) -> bool {
    let watching = decision.inputs.visibility == Visibility::Visible
        && decision.inputs.surface != Surface::Away;
    !overrides.skip_phone && !watching && !decision.plan.banner && !decision.plan.phone_card
}

/// Whether this event is the operator's RETURN, and so the moment a queued
/// notification can be put in front of them.
///
/// THE RETURN TRANSITION IS THE NEXT EVENT, and the engine has already
/// computed it. Nothing schedules a probe, so nothing OBSERVES a transition;
/// what the engine does do is read presence per event, at the last moment
/// before delivery, and publish the answer as the plan and the surface it
/// decided on. Both clauses below are values the record site already holds, so
/// this is no new probe, no second reading and no new trigger, and it inherits
/// the timing ruling for free.
///
/// AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED. The Away
/// row always cards, so without this clause the journal would be flushed at
/// the phone of an operator who has not come back, which is the opposite of
/// what "return" means.
///
/// THE DECORATION CLAUSE BUYS TWO PROPERTIES AND CODES NEITHER. A mute zeroes
/// the plan, so a muted run cannot flush the queue it is filling, and the
/// replay fires on the first event AFTER the mute lapses that earns the
/// operator something; nothing here reads `overrides.muted`, for the reason
/// `was_missed` reads the arbitrated plan rather than the matrix underneath
/// it. And a run whose plan decorated nothing is exactly a run that JOURNALS,
/// so a miss and a replay are mutually exclusive by construction: no event can
/// deliver the entry it just wrote.
///
/// IT IS THE ENGINE'S OWN PERCEPTION RULE RESTATED, not a second one. An
/// operator at the desk watching the origin pane earns nothing, live or
/// replayed, so the queue waits for an event on a pane they are not watching.
pub fn should_replay(decision: &Decision) -> bool {
    decision.inputs.surface != Surface::Away && (decision.plan.banner || decision.plan.phone_card)
}

/// Whether this event PROVES the operator was here, and so moves the recap
/// window's near edge forward.
///
/// AWAY IS THE ONLY THING THAT DOES NOT COUNT. Desk and Mobile are both a
/// human within reach of a screen; Away is the state the whole recap exists to
/// bracket, and the window it brackets runs from the last event that was not
/// one to now.
///
/// VISIBILITY IS DELIBERATELY NOT READ, unlike `was_missed`'s watching clause.
/// An operator at the desk looking at a different pane is still present, and
/// reading visibility here would make the window's near edge depend on which
/// pane happened to fire.
///
/// IT READS A VALUE THE DECISION ALREADY HOLDS, so it is no new probe and no
/// second reading, exactly as `was_missed` and `should_replay` are argued
/// above.
pub fn is_present(decision: &Decision) -> bool {
    decision.inputs.surface != Surface::Away
}

/// One journal entry: a single JSON object, on one line.
///
/// IT CARRIES THE FIVE VALUES `render::title` and `render::message` consume,
/// plus the epoch, and nothing else. Raw fields rather than a pre-rendered
/// string, because the replay may need to shape them differently from the
/// live card (one card per event, or one summary of several) and a frozen
/// string cannot be reshaped. Deliberately absent: the pane (an id from an
/// hour ago may name a pane that no longer exists, so a replayed card's click
/// would do nothing), the channel (the durable route already has the event),
/// the tier (it drove a pulse for work that is now over) and every leg
/// verdict (the decision ring is where delivery outcomes live).
///
/// JSON AND NOT THE RING'S key=value, because of the free text. A detail can
/// contain a newline, a quote or an escape byte, and one entry must stay one
/// line or an append forges a second entry. The ring solves that by refusing
/// free text; this cannot, so the escaping is taken from the library that is
/// already a dependency. BUILT WITH `json!` AND NEVER WITH `format!`, which
/// is the Rust spelling of this repo's "build JSON with `jq -n --arg`" rule:
/// interpolation is exactly how a newline in a detail would forge an entry.
///
/// `at` IS THE DECISION'S OWN CLOCK READ, never a second `SystemTime` call at
/// the record site, for the reason the decision log takes its epoch from
/// there: two readings of one moment can disagree. An unreadable clock writes
/// `null`, which is honest and which a reader can tell from an absent field.
///
/// `max_chars` IS THE CALLER'S, because two files now hold this shape and they
/// hold it for different readers. The journal passes the card's own cap, since
/// what a card renders without a cut is exactly what a replay needs; the
/// activity ring passes a timeline's cap, which is much shorter, because a
/// recap line is one line among a hundred and the full text of every event
/// already reached the durable log the recap points at. Neither number lives
/// here: this writes what it is given.
pub fn entry(event: &EventArgs, at: Option<u64>, max_chars: usize) -> String {
    let capped = |text: &str| crate::render::flatten_reply(text, max_chars);
    serde_json::json!({
        "at": at,
        "agent": capped(&event.agent),
        "state": capped(&event.state),
        "project": capped(&event.project),
        "branch": capped(&event.branch),
        "detail": capped(&event.detail),
    })
    .to_string()
}

/// The journal's contents read back into entries, oldest first, which is the
/// order the append leaves the file in.
///
/// PARSED BY KEY, never by position, which is what makes the writer's key
/// order (`serde_json`'s business, not this module's) invisible to the reader.
///
/// A LINE THAT IS NOT A JSON OBJECT IS SKIPPED, and it costs the rest of the
/// batch nothing. The file is a plain file in a directory an operator, a
/// backup tool or another program can reach, and the append's own heal can
/// republish a single line over it; one unparseable line must not throw away
/// the notifications around it. An object MISSING a field reads that field as
/// empty for the same reason: `render::title` and `render::message` already
/// have an answer for every empty value, so a short entry degrades to a
/// thinner card rather than to no card at all.
pub fn entries(contents: &str) -> Vec<Entry> {
    contents
        .lines()
        .filter_map(|line| {
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(line).ok()
        })
        .map(|fields| Entry {
            at: fields.get("at").and_then(serde_json::Value::as_u64),
            agent: text(&fields, "agent"),
            state: text(&fields, "state"),
            project: text(&fields, "project"),
            branch: text(&fields, "branch"),
            detail: text(&fields, "detail"),
        })
        .collect()
}

/// One text field off a parsed entry, absent and non-string alike reading as
/// empty.
fn text(fields: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod predicate_tests;

#[cfg(test)]
mod codec_tests;
