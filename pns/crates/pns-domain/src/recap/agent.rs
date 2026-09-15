//! The recap an agent wrote, fitted to one Discord message.
//!
//! THE AGENT COMPOSES, THIS ONLY FITS. The layout, the sections and every
//! sentence are the operator's and the agent's; what happens here is the same
//! three things the night recap's own budget does, in the one order the
//! layout's readability rules allow: make the characters safe, collapse the
//! file list to counts rather than truncating it, and drop whole sections from
//! the bottom of the priority order, counting what went.
//!
//! NOT `sections::body` AND NOT `budget::fit`. Those compose a body pns wrote
//! out of parts pns holds, and the cut they make clips individual lines to a
//! computed share. Run over a body somebody else wrote that would put half a
//! sentence in the channel, and run over the stack graph it would leave every
//! branch at the same depth. A line here survives whole or is counted.
//!
//! WHAT NEEDS THE OPERATOR IS NEVER SHED, which is `budget::fit`'s own
//! direction: `SHED_ORDER` names four sections and `**User Tasks**` is not one
//! of them.

use super::budget::{MAX_CHARS, remainder, spent};
use super::git_block::status_counts;
use super::sanitize::printable_line;

/// One agent-written recap, safe and under the character ceiling.
///
/// A BODY ALREADY UNDER THE CEILING COMES BACK AS IT WAS WRITTEN, sanitized
/// and no more. The layout tells the agent to keep the whole recap under 2,000
/// characters, so the ordinary case is the one that changes nothing.
pub fn fitted(recap: &str) -> String {
    let mut lines: Vec<String> = recap
        .lines()
        .map(|line| printable_line(line).trim_end().to_string())
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if fits(&lines) {
        return lines.join("\n");
    }
    collapse_file_list(&mut lines);
    for heading in SHED_ORDER {
        if fits(&lines) {
            break;
        }
        shed(&mut lines, heading);
    }
    lines.join("\n")
}

/// Whether the body is inside the ceiling the night recap posts under.
///
/// THE SAME CEILING, because it is the same gateway: `MAX_CHARS` is 1,800,
/// under the 1,900 the operator's hermes adapter splits a Discord message at
/// and well under Discord's own 2,000. A recap split in two is two messages.
fn fits(lines: &[String]) -> bool {
    spent(lines) <= MAX_CHARS
}

/// The file list inside the fenced block, replaced by its counts per status.
///
/// THE RULE IS THE LAYOUT'S OWN: "collapse a long file list to counts per
/// status rather than truncating mid-list". The rows go together or not at
/// all, and they are replaced where the first of them stood so the graph above
/// them keeps its place.
fn collapse_file_list(lines: &mut Vec<String>) {
    let rows: Vec<usize> = fenced(lines)
        .filter(|index| status_of(&lines[*index]).is_some())
        .collect();
    let (Some(first), true) = (rows.first().copied(), rows.len() > 1) else {
        return;
    };
    let counts = status_counts(rows.iter().filter_map(|index| status_of(&lines[*index])));
    for index in rows.into_iter().rev() {
        lines.remove(index);
    }
    lines.insert(first, counts);
}

/// The line numbers inside the body's first fenced block, which is where the
/// stack graph and the file list both live.
fn fenced(lines: &[String]) -> impl Iterator<Item = usize> + use<> {
    let fences: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == FENCE)
        .map(|(index, _)| index)
        .collect();
    match (fences.first(), fences.get(1)) {
        (Some(open), Some(close)) => (open + 1)..*close,
        _ => 0..0,
    }
}

/// The status letter of one file-list row, or none for any other line.
///
/// TWO SPACES ARE THE TEST, which is what tells a row from the counts line
/// that replaces it: `A  path/to/file` against `A 10  M 4`. Without it a
/// second pass would tally its own summary.
fn status_of(line: &str) -> Option<char> {
    let mut characters = line.chars();
    let letter = characters.next()?;
    let spacing: String = characters.by_ref().take(2).collect();
    (STATUS_LETTERS.contains(&letter) && spacing == "  " && characters.next().is_some())
        .then_some(letter)
}

/// One section's content lines replaced by the count of what went.
///
/// A SECTION GIVES UP ALL OF ITS LINES OR NONE OF THEM. The alternative is a
/// section half said, and a reader cannot tell that from a section that had
/// half as much to say; the count is what makes the cut honest.
fn shed(lines: &mut Vec<String>, heading: &str) {
    let Some(start) = lines.iter().position(|line| line == heading) else {
        return;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| is_heading(line))
        .map_or(lines.len(), |(index, _)| index);
    let dropped = lines[start + 1..end]
        .iter()
        .filter(|line| !line.is_empty())
        .count();
    let Some(counted) = remainder(dropped, false) else {
        return;
    };
    lines.splice(start + 1..end, [counted, String::new()]);
}

/// Whether a line opens a section of the layout.
fn is_heading(line: &str) -> bool {
    line.starts_with("**") && line.ends_with("**") && line.len() > 4
}

/// The order sections give up their lines in, least missed first.
///
/// `**User Tasks**` IS DELIBERATELY ABSENT. It is what the operator owes, the
/// one thing the recap exists to put in front of them, and `budget::fit`
/// protects its own equivalent for the same reason.
const SHED_ORDER: [&str; 4] = [
    "**Upcoming Agent Tasks**",
    "**In-Progress**",
    "**Summary**",
    "**Git**",
];
/// The status letters git's `--name-status` can put in the first column.
const STATUS_LETTERS: [char; 8] = ['A', 'C', 'D', 'M', 'R', 'T', 'U', 'X'];
/// The fence the stack graph and the file list share.
const FENCE: &str = "```";
