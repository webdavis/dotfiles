//! What a missed notification COMPOSES INTO: the card a replay delivers and
//! the line the doctor prints about the queue.
//!
//! POLICY ONLY. Every function here is a total function of its arguments,
//! with no config, no clock, no environment, no file and no printing.
//!
//! THE PRIVACY RULE, in one sentence: the journal holds what a CARD would
//! have shown, no pns command ever prints an entry, and the only thing that
//! reads an entry back is the replayer, which delivers it to the same
//! channels the live event would have reached. `waiting_line` is where that
//! rule is STRUCTURAL rather than promised: it counts non-empty lines and has
//! no parse, so there is no code path in it that can emit a field.
//!
//! The JSON codec stays in the adapter crate because this crate takes no
//! `serde_json`. The missed-event predicates live here beside composition,
//! using the decision values owned by this domain.

mod perception;
mod recap;
mod summary;

pub use perception::{is_present, should_replay, was_missed};
pub use recap::{NEEDS_YOU, event_count, needing_you, recap_card};
pub use summary::summary;

/// How many missed notifications the journal keeps.
///
/// TWENTY FIVE RATHER THAN THE RING'S FIVE. Five is argued from one
/// intervening Stop hook, which is a scale of seconds; this file has to
/// survive an absence of hours, and twenty five covers an evening at a few
/// notifiable events an hour. Unbounded is wrong for the other reason: this
/// is state, not a log stream, and nothing rotates it.
///
/// RAISING IT IS THIS ONE NUMBER ONLY UP TO A CEILING, and the ceiling is
/// near enough to state. Each of the five text fields is capped at
/// `render::PREVIEW_MAX_CHARS` characters, and one character can cost six
/// bytes escaped (a control byte is written `\u001b`), so a worst-case entry
/// MEASURES 7,876 bytes and a full journal 196,900, which is 75% of the 256
/// KiB the composition root reads any of these state files back through.
/// Past a depth of 33 a full journal no longer reads back at all, and the
/// append answers a file it cannot read by republishing the one line it just
/// wrote: the journal would collapse to a single entry exactly when it is
/// fullest, and silently. Raising this past 33 means raising that read cap in
/// the same change.
///
/// ORDINARY ENTRIES ARE NOWHERE NEAR THAT, a few hundred bytes of plain text,
/// so the ceiling is reached only by fields that are all escape bytes. It is
/// stated because the collapse is silent, not because it is likely.
pub const KEPT: usize = 25;

/// One journal entry read back: the six values `entry` wrote, and nothing
/// else.
///
/// THE READ SIDE OF `entry`, kept beside it so the pair changes together. It
/// is a struct rather than a `serde_json::Value` because the replay renders
/// from it and a caller holding a `Value` would be free to reach for a key
/// nobody wrote.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// The epoch the miss was journaled at, absent when the writer had no
    /// readable clock.
    pub at: Option<u64>,
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
}

/// The doctor's one line about the journal, from the file's contents.
/// `contents` is `None` when there is no journal at all.
///
/// IT COUNTS AND NEVER PARSES, and that is the privacy rule made structural
/// rather than promised: there is no code path in here that could emit a
/// field, because nothing in here ever looks inside a line. Anyone tempted to
/// make this "more helpful" by rendering the newest entry is about to print
/// the operator's own text to a terminal, which is exactly what the decision
/// ring refuses free text to avoid.
///
/// IT SAYS WHAT IS WAITING, never "you missed N". The prune drops the oldest,
/// so over a long absence the file under-reports what was truly missed, and no
/// line here claims a number the file cannot back.
///
/// IT NAMES WHAT DELIVERS THEM, which is a promise the binary keeps, and it
/// names it EXACTLY. The sentence used to end "nothing replays them yet",
/// which the replay made false the moment it shipped, and then "the next
/// event the operator is present for", which promises more than the binary
/// does: presence alone delivers nothing. Three things have to be true at
/// once, and the sentence says all three. The operator is not away; the event
/// earned a banner or a card (a muted one earns neither, and neither does one
/// on a pane they are watching); and a leg was there to raise it (a machine
/// with only a durable channel raises nothing). The zero case says nothing
/// about replaying, because there is nothing waiting to promise anything
/// about.
///
/// `replay_card` IS THE FOURTH THING, and the one no event can satisfy: with
/// `[recap] replay_card = false` the delivery is switched off, so the promise
/// above is one the binary cannot keep for as long as the switch stands. The
/// off sentence says what is true instead, which is that the misses are
/// RECORDED (the journal writes regardless of the switch) and that nothing
/// moves them until the card is switched back on. The zero case is the same
/// sentence either way: there is nothing waiting, so there is nothing to
/// promise or unpromise about.
pub fn waiting_line(contents: Option<&str>, replay_card: bool) -> String {
    let waiting = contents
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    match waiting {
        0 => NONE_WAITING.to_string(),
        1 if replay_card => "pns doctor: 1 missed notification is waiting to be replayed; \
             the next event that raises a banner or a card while the operator \
             is not away delivers it."
            .to_string(),
        1 => "pns doctor: 1 missed notification is recorded; the catch-up card \
             is switched off (`[recap] replay_card = false`), so nothing delivers \
             it until the card is switched back on."
            .to_string(),
        many if replay_card => {
            format!(
                "pns doctor: {many} missed notifications are waiting to be replayed; \
                 the next event that raises a banner or a card while the operator \
                 is not away delivers them."
            )
        }
        many => {
            format!(
                "pns doctor: {many} missed notifications are recorded; the catch-up card \
                 is switched off (`[recap] replay_card = false`), so nothing delivers \
                 them until the card is switched back on."
            )
        }
    }
}

/// An empty journal, which is honestly ambiguous: either nothing was missed or
/// a write did not land. It says what is RECORDED for that reason, and claims
/// neither reading.
const NONE_WAITING: &str = "pns doctor: no missed notification is recorded.";

#[cfg(test)]
#[path = "missed/tests/predicates.rs"]
mod predicate_tests;

#[cfg(test)]
mod tests;
