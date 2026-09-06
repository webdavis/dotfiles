//! What a recap body is made of, and the order the sections run in.

use super::budget::Trim;
use super::budget::{Clock, MAX_LINES, fit};
use super::external::Externals;
use super::external::LINE_PREFIX;
use super::external::external_section;
use super::night::{described, night_section};
use super::prompt::TAIL;
use super::prompt::{MERGES, NOTES};
use crate::missed::{Entry, needing_you};

/// One part of the body, and whether the budget may cut it.
///
/// THE HEADING IS THE FIRST LINE, which is what makes trimming a tail slice:
/// a cut section keeps its heading, as many lines as fit, and one line naming
/// the true remainder.
#[derive(Debug, PartialEq)]
pub struct Section {
    pub lines: Vec<String>,
    pub trim: Trim,
    /// What this section left out BEFORE the budget ever saw it: sources no
    /// surviving line speaks for. `fit` adds whatever it cuts itself and
    /// renders one remainder line for the sum, so a section cannot end up
    /// carrying two counts that disagree.
    pub omitted: usize,
    /// Whether `omitted` is a FLOOR rather than a total, which is what a cap
    /// on the fetch leaves behind: pns stopped reading, so it cannot say how
    /// many more there were.
    pub at_least: bool,
}
impl Section {
    /// A section the budget may not cut, with nothing left out of it.
    pub(super) fn held(lines: Vec<String>) -> Section {
        Section {
            lines,
            trim: Trim::Never,
            omitted: 0,
            at_least: false,
        }
    }
}
/// What section 3 is made of, and the ONLY thing a summarizer can change.
///
/// THE SUBSTITUTION POINT IS A TYPE rather than a rule in a prompt, which is
/// what makes SELECTION-NOT-RECONSTRUCTION structural: the header's count, what
/// needs the operator, and every other section are composed the same way
/// whichever variant this is, so a model that answered with a different count
/// or with nothing urgent in it cannot move either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Timeline<'lines> {
    /// One line per event, composed here. The setting of a machine with no
    /// summarizer configured, and the floor every other outcome falls to.
    Mechanical,
    /// What the summarizer said, already flattened and capped by `answer`.
    Summarized(&'lines [String]),
    /// A summarizer was configured and did not answer. The mechanical lines,
    /// and one line saying that is what they are.
    Unanswered,
}
/// The v3 body, in order: the window header, what needs the operator, the
/// night in order, what shipped, what review caught, and the pointer to where
/// the full text lives.
///
/// SECTIONS 4 AND 5 SAY WHICH OF THEIR STATES THEY ARE IN rather than being
/// omitted. A section that vanished would be indistinguishable from a night
/// with no merges and no review findings, which is a different claim entirely,
/// and so is a source that would not answer.
///
/// AND SO DOES A SUMMARIZER THAT WENT QUIET, for the same reason: the plain
/// list of a night nobody was asked to summarize and the plain list of a model
/// that timed out read identically otherwise, and only one of them is worth
/// looking into. THAT ONE IS SAID BY `night_section`, in the heading of the
/// very list it is about, so no arrangement of the budget can leave the note
/// standing over a night the message does not carry.
pub fn sections(
    entries: &[Entry],
    from: &str,
    to: &str,
    clock: Clock,
    timeline: Timeline,
    externals: &Externals,
) -> Vec<Section> {
    let mut parts = vec![
        Section::held(vec![header(entries.len(), from, to)]),
        needs_you_section(entries),
        night_section(entries, clock, timeline),
    ];
    parts.extend([
        external_section(&MERGES, &externals.merges),
        external_section(&NOTES, &externals.notes),
        Section::held(vec![TAIL.to_string()]),
    ]);
    parts
}
/// The whole body, fitted to the budget and joined into one message.
pub fn body(
    entries: &[Entry],
    from: &str,
    to: &str,
    clock: Clock,
    timeline: Timeline,
    externals: &Externals,
) -> String {
    fit(
        &sections(entries, from, to, clock, timeline, externals),
        MAX_LINES,
    )
    .join("\n")
}
/// The first line, which is also the thread's title when the route is a forum
/// channel: hermes names a new forum thread after the message's first line.
///
/// THE COUNT IS THE ENTRIES THAT WERE READ, never the ones that survived the
/// budget and never a claim about everything that happened. The ring prunes to
/// its own depth, so over a very long absence this is a floor rather than a
/// total, which is the same honesty `waiting_line` states about the journal.
///
/// AND IT IS THE CARD'S OWN SENTENCE, from `event_count`, so the two layers of
/// one return cannot pluralize the same number two ways.
///
/// THIS IS THE CHILD'S READ OF THE RING and the card's count was the parent's,
/// so an event landing between them leaves the two one apart. Each is honest
/// about what IT read; `spawn_recap` states why nothing reconciles them.
pub(super) fn header(counted: usize, from: &str, to: &str) -> String {
    format!(
        "While you were away, {from}-{to} · {}",
        crate::missed::event_count(counted)
    )
}
/// What is still waiting on the operator, newest first and never cut.
pub(super) fn needs_you_section(entries: &[Entry]) -> Section {
    let waiting = needing_you(entries);
    let mut lines = vec![NEEDS_YOU_HEADING.to_string()];
    if waiting.is_empty() {
        lines.push(NOTHING_WAITING.to_string());
    } else {
        lines.extend(
            waiting
                .iter()
                .rev()
                .map(|entry| format!("{LINE_PREFIX}{}", described(entry))),
        );
    }
    Section::held(lines)
}
pub(super) const NEEDS_YOU_HEADING: &str = "NEEDS YOU";
/// Said rather than left blank: an empty section reads as a section that
/// broke, and this one is the reason the message exists.
pub(super) const NOTHING_WAITING: &str = "- nothing is waiting on you";
