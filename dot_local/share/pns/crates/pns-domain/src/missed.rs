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
//! The JSON codec that writes and reads an entry stays in the legacy package,
//! because this crate takes no `serde_json`, and so do the three predicates
//! that decide whether an event was missed, because they answer over the
//! engine's `Decision`.

use crate::decision::{Decision, Overrides};
use crate::surface::{Surface, Visibility};

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

/// The one card a replay delivers, whatever the count: the true count, then
/// as many entries as fit, NEWEST FIRST.
///
/// ONE SHAPE AND NO SPECIAL CASE. A summary for many plus the real card for
/// exactly one would be two code paths, two sets of tests and a seam where
/// the two can disagree about what a replayed card looks like; and a
/// one-entry summary carries the same content the real card would, because
/// an entry holds exactly the values `render::title` and `render::message`
/// consume.
///
/// `waiting` ARRIVES IN THE FILE'S OWN ORDER, oldest first, and is rendered
/// newest first here, because `render::preview` cuts from the START: what
/// survives a cut has to be what matters most.
///
/// THE COUNT IS ALWAYS THE REAL COUNT, even when the body stopped early, so
/// the card never claims a number it did not show and never shows a number it
/// cannot back. The body stops at `render::PREVIEW_MAX_CHARS` rather than
/// leaving the cut to `preview`, so the operator is told how many are behind
/// the ones they can read; the full text of every entry already reached the
/// durable log when it happened.
///
/// THE NEWEST ENTRY GOES IN WHATEVER ITS LENGTH, and only the ones behind it
/// have to fit. MEASURED: a single missed notification with a 209-character
/// detail took the body one character past the cap, so the loop stopped
/// before appending anything and the card read "1 missed notification" with
/// no content at all, which is precisely the notification it exists to
/// deliver. The cut for that one entry is `render::preview`'s, on the way
/// out, which is where every other over-long body is already cut.
pub fn summary(waiting: &[Entry]) -> String {
    let mut body = match waiting.len() {
        1 => "1 missed notification".to_string(),
        many => format!("{many} missed notifications"),
    };
    for (shown, entry) in waiting.iter().rev().enumerate() {
        let separator = if shown == 0 { ". " } else { "; " };
        let extended = format!("{body}{separator}{}", rendered(entry));
        // STOPPED RATHER THAN SKIPPED, which is also what lets the index above
        // stand in for how many were shown: the entries left out are the
        // oldest, and a body that skipped a long one to reach an older short
        // one would read as though the newest were missing.
        //
        // AND NEVER BEFORE THE FIRST ONE. `shown == 0` is the newest entry
        // with nothing appended yet, and stopping there leaves the count
        // standing alone as the whole card.
        if shown > 0 && extended.chars().count() > crate::render::PREVIEW_MAX_CHARS {
            break;
        }
        body = extended;
    }
    body
}

/// The states that mean an agent is WAITING ON THE OPERATOR rather than
/// reporting to them.
///
/// ONE LIST, TWO READERS. The phone card's needs-you line and the recap's own
/// NEEDS YOU section are the same question asked at two sizes, and two copies
/// of this list would drift the day a sixth state joins. The first four are the
/// mid-turn arm's own words in the composition root; `failed` is a turn that
/// died, which needs the operator every bit as much as one that asked.
pub const NEEDS_YOU: [&str; 5] = ["asked", "blocked", "denied", "failed", "plan-ready"];

/// The entries in a window that still need the operator, in the order they
/// arrived.
pub fn needing_you(entries: &[Entry]) -> Vec<Entry> {
    entries
        .iter()
        .filter(|entry| NEEDS_YOU.contains(&entry.state.as_str()))
        .cloned()
        .collect()
}

/// The phone layer of the return recap: what still needs the operator, then
/// the true counts, then where the rest is.
///
/// NEEDS YOU FIRST AND NEVER SUMMARIZED AWAY, which is why it is composed here
/// and not by any model: the urgent line is the one a hallucination would cost
/// the most, and it is the one thing on this card that cannot wait for the
/// Discord recap to be read.
///
/// EVERY NUMBER IS A LENGTH, never a claim. `counted` is the window's own
/// length and `missed` is the claimed journal's, so a card that ran out of room
/// still names totals it can back. That is `summary`'s count-never-lies rule
/// applied to a second card.
///
/// THE COUNTS AND THE POINTER ARE RESERVED, and the urgent items are fitted
/// into whatever room is left. MEASURED as the reason this is not a stop rule
/// alone: a 120-character agent and a 120-character project compose a
/// 253-character title, the first urgent item used to go in whatever its
/// length, and the card reached 289 characters. `render::preview` is what the
/// phone is actually handed, and it cuts at the last SENTENCE END that fits,
/// which is the full stop before the counts: the delivered preview was
/// 254 characters of title with the event count, the missed count and the
/// pointer all gone. So the newest urgent item is CUT to the room rather than
/// dropped (a card without the one thing waiting on the operator is the
/// notification it exists to deliver) and the card never exceeds the cap at
/// all, which is what makes the preview a no-op.
///
/// TWO INDEPENDENT READS OF ONE RING, STATED. `counted` is this process's read
/// of the window and the Discord header is the child's, so the two can differ
/// by an event written between them. Each is honest about what it read; see
/// `spawn_recap`'s own comment for why nothing reconciles them.
///
/// "recap in #pns" IS ONLY SAID WHEN THERE IS ONE. `digest_posted` is whether a
/// child was really started, not whether one was wanted, so the card never
/// points at a recap that was never going to arrive.
pub fn recap_card(
    needs_you: &[Entry],
    counted: usize,
    missed: usize,
    digest_posted: bool,
) -> String {
    let mut counts = event_count(counted);
    if missed > 0 {
        counts.push_str(&format!(", {missed} missed"));
    }
    if digest_posted {
        counts.push_str(". recap in #pns");
    }
    // THE ROOM THE COUNTS LEFT, separator included, which is what every urgent
    // item is fitted into. A count so long that nothing is left is an empty
    // room, and the card is then the counts alone.
    let room = crate::render::PREVIEW_MAX_CHARS.saturating_sub(counts.chars().count() + SEPARATOR);
    let mut urgent: Vec<String> = Vec::new();
    for entry in needs_you.iter().rev() {
        let mut extended = urgent.clone();
        extended.push(crate::render::clipped(
            &crate::render::title(&entry.agent, &entry.state, &entry.project),
            room,
        ));
        // STOPPED RATHER THAN SKIPPED, and never before the first: `summary`'s
        // own two rules, for its own two reasons. The first item is already
        // inside the room by the clip above, so "never before the first" costs
        // the cap nothing here.
        if !urgent.is_empty() && joined(&extended).chars().count() > room {
            break;
        }
        urgent = extended;
    }
    with_counts(&urgent, &counts)
}

/// The window's own count, said ONCE so the phone card and the Discord header
/// cannot disagree about it. A one-event window read "1 events" on the phone
/// and "1 event" in Discord while this was two sentences.
pub fn event_count(counted: usize) -> String {
    if counted == 1 {
        "1 event".to_string()
    } else {
        format!("{counted} events")
    }
}

/// What separates the urgent items from the counts, counted rather than
/// guessed at, so the reservation above and the composition below cannot
/// disagree about its width.
const SEPARATOR: usize = ". ".len();

/// The urgent items in front of the counts, or the counts alone.
fn with_counts(urgent: &[String], counts: &str) -> String {
    if urgent.is_empty() {
        counts.to_string()
    } else {
        format!("{}. {counts}", joined(urgent))
    }
}

/// The urgent items as one run of text, which is the thing the room is
/// measured against.
fn joined(urgent: &[String]) -> String {
    urgent.join("; ")
}

/// One entry as a line of the summary: the card's own title, and its text
/// where there is any.
///
/// THE TITLE ALONE FOR AN EMPTY DETAIL, because the title already carries the
/// state a bare `done` turn would otherwise repeat after a colon.
fn rendered(entry: &Entry) -> String {
    let title = crate::render::title(&entry.agent, &entry.state, &entry.project);
    if entry.detail.is_empty() {
        title
    } else {
        format!("{title}: {}", entry.detail)
    }
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

#[cfg(test)]
mod predicate_tests;

#[cfg(test)]
mod tests;
