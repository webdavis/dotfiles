//! The night's own section: what happened while nobody was looking.

use super::budget::{Clock, Trim};
use super::external::LINE_PREFIX;
use super::prompt::SUMMARIZER_SILENT;
use super::sections::Section;
use super::sections::Timeline;
use crate::missed::Entry;

/// The night itself, oldest first, one line per event. THE ONLY TRIMMABLE
/// SECTION, because it is the only one whose length follows the window's, and
/// the only one a summarizer is allowed to write.
///
/// A SUMMARIZED SECTION IS TRIMMABLE TOO, which is the budget re-applied to the
/// model's own answer: a backend that ignored the line count it was asked for
/// is cut here exactly as a long night is.
///
/// AND EVERY SUMMARIZED LINE IS PREFIXED AND COUNTED, which is what makes the
/// substitution structural rather than hopeful. A line whose whole text is
/// `NEEDS YOU` or a second window header is ordinary printable text that
/// `safe_line` has no reason to touch, so it used to be spliced through and
/// render AS a heading; a leading `- ` costs two characters of the width and
/// makes that impossible, exactly as a mechanical line's own `HH:MM {mark} `
/// does. And an answer longer than the window it summarizes is cut to the
/// window's length, because `fit`'s remainder counts this section's lines: over
/// a thirteen-event window a two-hundred-line answer would otherwise end in
/// "...and 183 more" under a header saying 13 events, and COUNT NEVER LIES is
/// the rule this whole module is arranged around. Nothing past the window's own
/// length was ever an event.
///
/// THE QUIET SUMMARIZER IS NAMED IN THE HEADING rather than in a section of its
/// own, so the note and the list it describes cannot be separated. As a
/// protected section it outlived a night the budget dropped WHOLE, and the only
/// list left above it was NEEDS YOU: the message then said the plain list was
/// plain about a night it did not carry at all. In the heading it also costs
/// the timeline nothing, where a line of its own cost it one.
pub(super) fn night_section(entries: &[Entry], clock: Clock, timeline: Timeline) -> Section {
    if entries.is_empty() {
        // SAID RATHER THAN LEFT BLANK, for `NOTHING_WAITING`'s own reason: a
        // heading with nothing under it reads as a section that broke. IT
        // ANSWERS EVERY VARIANT, because a window with no events has no night
        // for anybody to have summarized.
        return Section::held(vec![
            NIGHT_HEADING.to_string(),
            NOTHING_HAPPENED.to_string(),
        ]);
    }
    if let Timeline::Summarized(lines) = timeline {
        let mut summarized = vec![NIGHT_HEADING.to_string()];
        summarized.extend(
            lines
                .iter()
                .take(entries.len())
                .map(|line| format!("{LINE_PREFIX}{line}")),
        );
        return Section {
            lines: summarized,
            trim: Trim::Always,
            omitted: 0,
            at_least: false,
        };
    }
    let mut lines = vec![match timeline {
        Timeline::Unanswered => format!("{NIGHT_HEADING} {SUMMARIZER_SILENT}"),
        _ => NIGHT_HEADING.to_string(),
    }];
    lines.extend(entries.iter().map(|entry| {
        format!(
            "{} {} {}",
            clock(entry.at),
            mark(&entry.state),
            described(entry)
        )
    }));
    Section {
        lines,
        trim: Trim::Always,
        omitted: 0,
        at_least: false,
    }
}
/// One event as a line of the timeline: who, in what state, on what, and what
/// it said.
///
/// NO DANGLING PUNCTUATION for an entry that carries no project or no detail,
/// which is `rendered`'s own rule in the journal: a line ending in a colon
/// reads as truncated rather than complete.
pub(super) fn described(entry: &Entry) -> String {
    let agent = if entry.agent.is_empty() {
        "pns"
    } else {
        &entry.agent
    };
    let state = if entry.state.is_empty() {
        "done"
    } else {
        &entry.state
    };
    let mut line = format!("{agent}/{state}");
    if !entry.project.is_empty() {
        line.push(' ');
        line.push_str(&entry.project);
    }
    if !entry.detail.is_empty() {
        line.push_str(": ");
        line.push_str(&entry.detail);
    }
    line
}
/// The timeline's marker for one state word, chosen MECHANICALLY and never by
/// a model: a finished turn, a turn that died, and a turn waiting on the
/// operator are the three the eye is scanning for, and everything else is
/// deliberately unmarked so those three stand out.
pub(super) fn mark(state: &str) -> &'static str {
    if crate::missed::NEEDS_YOU.contains(&state) {
        // `failed` is in that list AND is its own kind of news, so it is
        // answered before the waiting mark rather than inside it.
        return if state == "failed" { "!" } else { "?" };
    }
    if state == "done" { "+" } else { " " }
}
pub(super) const NIGHT_HEADING: &str = "THE NIGHT IN ORDER";
/// An empty window, which the event path never posts (nothing is under every
/// threshold) and a hand-run `pns recap` reaches whenever it is pointed at a
/// quiet stretch.
pub const NOTHING_HAPPENED: &str = "- nothing was recorded in this window";
