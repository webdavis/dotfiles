//! The two budgets a body is composed under, and what a cut takes first.

use super::sections::Section;

/// The hard ceiling on how many lines one recap message spends.
///
/// TWENTY FIVE, as locked. It is a budget rather than a hope: `fit` enforces
/// it, and the one thing allowed to exceed it is a NEEDS YOU list longer than
/// the budget itself, because a recap that dropped what is waiting on the
/// operator has failed at the one job the phone card could not do for it.
pub const MAX_LINES: usize = 25;
/// The hard ceiling on how many CHARACTERS one recap message spends.
///
/// EIGHTEEN HUNDRED, under the 1,900 the operator's own hermes adapter splits
/// a Discord message at, which is itself under Discord's 2,000. The locked
/// spec is ONE message, and a body split in two is two: on the plain route
/// that is two messages in the channel, and on the forum route the tail lands
/// as follow-ups behind the thread starter. MEASURED on the real binary before
/// this existed: forty entries at the ring's own field cap rendered as
/// 2,859 characters inside the 25-line budget.
///
/// A HUNDRED CHARACTERS OF HEADROOM is deliberate, and it is NOT an encoding
/// allowance. VERIFIED in the operator's own gateway: the Discord adapter sets
/// `MAX_MESSAGE_LENGTH = 2000` and `_SPLIT_THRESHOLD = 1900` and spends both
/// through Python's `len` on a `str`, so both ceilings count CHARACTERS and
/// 1,800 of them is under 1,900 whatever the encoding. The gap covers the one
/// line a caller may append to a composed body (`THREAD_UNAVAILABLE`) and the
/// gateway moving its own threshold, not multi-byte text.
pub const MAX_CHARS: usize = 1_800;
/// The local wall clock as a total function this module is HANDED rather than
/// one it reads. `Some(epoch)` becomes `HH:MM`; an unknown epoch becomes a
/// placeholder of the same width, so a line with no clock still lines up under
/// the ones that have one.
pub type Clock<'clock> = &'clock dyn Fn(Option<u64>) -> String;
/// When the budget may cut a section.
///
/// THREE ANSWERS RATHER THAN TWO, because the two external sections are
/// neither. They are worth their whole reservation on an ordinary window and
/// they are the first thing to give when a loud one would not otherwise fit in
/// one message, so they are cut LAST and only when something has to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trim {
    /// Never. NEEDS YOU, the header, the tail, and any section that is one
    /// sentence long: cutting those buys nothing and loses what they had.
    Never,
    /// Only when the message would otherwise break a budget. The section
    /// keeps its heading and its remainder line, which is what makes the cut
    /// honest rather than silent.
    WhenOver,
    /// The night, which is the section whose length follows the window's and
    /// therefore the one a cut is meant for.
    Always,
}
/// At most `budget` lines AND at most `MAX_CHARS` characters, cutting only
/// what may be cut.
///
/// THE PROTECTED SECTIONS ARE RESERVED FIRST, so what is left over is what the
/// trimmable ones may spend. A protected total larger than either budget is
/// allowed to exceed it, which is the deliberate direction: NEEDS YOU is never
/// what gets dropped, and a recap that is a few lines long is a message the
/// operator still reads.
///
/// A CUT SECTION KEEPS ITS HEADING and ends with the TRUE remainder, counted
/// against the section's own length rather than against what survived, so the
/// "and N more" cannot disagree with the header's count.
///
/// TWO BUDGETS, BECAUSE A LINE HAS A LENGTH. MEASURED on the real binary:
/// forty entries at the ring's own 120-character field cap render as
/// 2,859 characters inside 25 lines, and the gateway splits a Discord message
/// at 1,900. The locked spec is ONE message, so the line budget alone was
/// never enough. The characters come off each surviving line's own SHARE
/// rather than off the number of lines, because a timeline that says less
/// about more is still the night in order; a timeline missing its second half
/// is not.
pub fn fit(sections: &[Section], budget: usize) -> Vec<String> {
    let (whole, starved) = lay_out(sections, budget, false);
    // THE SECOND PASS IS THE ONLY THING THAT CUTS THEM, so an ordinary window
    // carries both external sections entire and only a window that would break
    // a budget, or leave the night nothing at all, pays for them.
    if !starved && whole.len() <= budget && spent(&whole) <= MAX_CHARS {
        return whole;
    }
    lay_out(sections, budget, true).0
}
/// One pass of the budget over the sections, and whether it left a section that
/// had something to say saying none of it.
///
/// THE NIGHT IS WHAT A CUT IS FOR and it is still cut first; `over` is what
/// happens when cutting it was not enough, or when there was no night left to
/// cut. Then the two external sections give up their content lines to the
/// remainder they already carry, which costs the message a fact it can still
/// count and buys back the room the night needed.
pub(super) fn lay_out(sections: &[Section], budget: usize, over: bool) -> (Vec<String>, bool) {
    let reserved: Vec<Option<Vec<String>>> = sections
        .iter()
        .map(|section| (section.trim != Trim::Always).then(|| held_lines(section, over)))
        .collect();
    let lines_reserved: usize = reserved.iter().flatten().map(Vec::len).sum();
    // THE SEPARATOR COUNTS: `body` joins with newlines, so a line costs its own
    // length plus one. Counting one for the last line too leaves the ceiling a
    // character under rather than a character over.
    let chars_reserved: usize = reserved
        .iter()
        .flatten()
        .map(|lines| spent(lines.as_slice()))
        .sum();
    let mut room = budget.saturating_sub(lines_reserved);
    let mut fitted = Vec::new();
    let mut starved = false;
    for (section, held) in sections.iter().zip(&reserved) {
        if let Some(lines) = held {
            fitted.extend(lines.iter().cloned());
            continue;
        }
        let content = section.lines.len() - 1;
        // TWO LINES ARE THE FLOOR for a section that has to be cut: its own
        // heading, and the line naming what was left out. With less room than
        // that the section says nothing at all, and the header's count is
        // still the whole window.
        let mut shown = if section.lines.len() <= room {
            content
        } else if room < 2 {
            room = 0;
            starved |= content > 0;
            continue;
        } else {
            room - 2
        };
        // EVERY OTHER LINE IN THE MESSAGE IS ALREADY SPOKEN FOR by the time
        // the share is worked out: the reserved sections, this section's own
        // heading, and the remainder line if there is one. What is left is
        // divided evenly, LESS THE NEWLINE each surviving line costs on top of
        // its own text. A line already under its share simply keeps its whole
        // length, which leaves the total under the ceiling rather than on it.
        let share = |shown: usize| {
            let spoken_for = chars_reserved
                + spent(&section.lines[..1])
                + remainder(content - shown + section.omitted, section.at_least)
                    .map_or(0, |line| spent(&[line]));
            (MAX_CHARS.saturating_sub(spoken_for) / shown.max(1)).saturating_sub(1)
        };
        // A LINE WITH NO ROOM IS NOT A LINE. `clipped` to nothing is the empty
        // string, so a share of zero renders blank lines under a heading, which
        // is worse than the heading and a truthful count on their own: the
        // reader learns the same nothing and cannot tell the message was cut.
        if share(shown) == 0 {
            shown = 0;
        }
        starved |= shown == 0 && content > 0;
        let width = share(shown);
        fitted.push(section.lines[0].clone());
        fitted.extend(
            section.lines[1..=shown]
                .iter()
                .map(|line| crate::render::clipped(line, width)),
        );
        fitted.extend(remainder(
            content - shown + section.omitted,
            section.at_least,
        ));
        room = room.saturating_sub(section.lines.len());
    }
    (fitted, starved)
}
/// One section as this pass renders it, for every section the budget is not
/// dividing its room between: whole, or cut to its heading and the count of
/// everything it then owes.
pub(super) fn held_lines(section: &Section, over: bool) -> Vec<String> {
    let cut = section.trim == Trim::WhenOver && over;
    let content = section.lines.len() - 1;
    let mut lines = match cut {
        true => vec![section.lines[0].clone()],
        false => section.lines.clone(),
    };
    lines.extend(remainder(
        if cut { content } else { 0 } + section.omitted,
        section.at_least,
    ));
    lines
}
/// The line naming what a section left out, or nothing when it left out
/// nothing. AT LEAST is what a count says when a cap stopped the fetch itself:
/// the number is what pns can prove, and the phrase is what stops it reading as
/// a total.
pub(super) fn remainder(dropped: usize, at_least: bool) -> Option<String> {
    (dropped > 0).then(|| match at_least {
        true => format!("...and at least {dropped} more"),
        false => format!("...and {dropped} more"),
    })
}
/// What a run of lines costs the character budget: its own text plus the
/// newline that joins it to the next one.
pub(super) fn spent(lines: &[impl AsRef<str>]) -> usize {
    lines
        .iter()
        .map(|line| line.as_ref().chars().count() + 1)
        .sum()
}
