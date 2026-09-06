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

pub use pns_domain::missed::{
    Entry, KEPT, NEEDS_YOU, event_count, is_present, needing_you, recap_card, should_replay,
    summary, waiting_line, was_missed,
};

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
mod codec_tests;
