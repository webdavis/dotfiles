use crate::KEPT;

mod summary;
pub use summary::{Summary, summarize};

const NO_CLOCK: &str = "-";

/// How much of an entry the reader wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// A sentence per decision: what happened, and the one reason for it.
    Spoken,
    /// The line as the ring wrote it, every field. `pns doctor --raw`.
    Raw,
}

/// The decision log as the doctor's own section: a heading and one rendered
/// entry per line, newest first, capped at `KEPT`.
///
/// `contents` is the file, `None` when there is none. `now` is the reader's
/// own clock, which ages the entries and nothing else.
///
/// IT REPORTS HISTORY, NEVER HEALTH. Nothing here reaches an exit code: an
/// empty log on a fresh machine is not a failure, and neither is one this
/// could not parse.
pub fn section(
    contents: Option<&str>,
    now: Option<u64>,
    detail: Detail,
) -> Vec<(super::Mark, String)> {
    let entries: Vec<&str> = contents
        .unwrap_or_default()
        .lines()
        .filter(|entry| !entry.trim().is_empty())
        .collect();
    if entries.is_empty() {
        return vec![(super::Mark::Note, NOTHING_RECORDED.to_string())];
    }
    // NEWEST FIRST, because the ring is written by APPEND and the operator
    // came to look at the card that just did or did not arrive.
    let shown: Vec<&&str> = entries.iter().rev().take(KEPT).collect();
    let mut rendered = vec![(super::Mark::Note, heading(shown.len(), detail))];
    for entry in shown {
        rendered.extend(render(entry, now, detail));
    }
    rendered
}

/// The section's own first line, counting what it is ABOUT TO SHOW rather than
/// the cap: a heading claiming five over one entry invents four decisions.
fn heading(shown: usize, detail: Detail) -> String {
    let counted = if shown == 1 {
        "the last decision".to_string()
    } else {
        format!("the last {shown} decisions")
    };
    // THE TAIL IS THE POINTER, not a caveat. The actionId note that used to
    // live here explained an absence nobody had asked about, in front of the
    // entries somebody had; it is a comment on `line` now, where the field
    // would have been written.
    match detail {
        // NEVER "NEWEST FIRST" HERE. The section blurb one line above already
        // says it, and a reader meeting the same three words twice in two lines
        // learns that this line is not worth reading.
        Detail::Spoken => format!("pns doctor: {counted} (`--raw` for every input behind them)"),
        Detail::Raw => format!("pns doctor: {counted}, every input behind them"),
    }
}

/// One entry, as its age and the rest of the line it was written as.
///
/// THE BODY IS NEVER PARSED, only ESCAPED and printed. The whole reader is
/// the one split below, which is what keeps a format change in `line` from
/// needing a matching change here.
fn render(entry: &str, now: Option<u64>, detail: Detail) -> Vec<(super::Mark, String)> {
    let Some((stamp, rest)) = entry.split_once(' ') else {
        return vec![(super::Mark::Detail, complaint(entry))];
    };
    let recorded = if stamp == NO_CLOCK {
        None
    } else {
        match crate::count::parse_count(stamp) {
            Some(recorded) => Some(recorded),
            // NOT DROPPED SILENTLY: a log that hides the one entry that
            // mattered is worse than one that says it cannot read it.
            None => return vec![(super::Mark::Detail, complaint(entry))],
        }
    };
    let when = age(recorded, now);
    if detail == Detail::Raw {
        return vec![(super::Mark::Detail, format!("{when}: {}", escaped(rest)))];
    }
    // THE BODY IS ESCAPED BEFORE IT IS READ, not after. Every value in the
    // ring is a number, a boolean or a name off the compiled roster, so
    // escaping cannot change what a field means, and doing it first means no
    // path here can put an unescaped fragment of the file on a terminal.
    let spoken = summarize(&escaped(rest));
    let mut lines = vec![(
        super::Mark::Detail,
        format!("{when}: {} {}", spoken.event, spoken.outcome),
    )];
    if let Some(because) = spoken.because {
        lines.push((super::Mark::Aside, because));
    }
    lines
}

/// THE ONE ESCAPE RULE for text out of the ring, and the reason it is a
/// function rather than two spellings: a parsed entry and an unparsable one
/// go to the SAME terminal, so a rule applied to only one of them is a rule
/// the other arm quietly does not have. Measured before this existed: an
/// entry whose epoch parsed printed its ESC and BEL bytes to the terminal
/// raw, while its unparsable neighbour on the line above was escaped.
///
/// Rust's own debug escaping is the rule, without the quotes `complaint`
/// wraps its half in: nothing here writes a format, it makes a control byte
/// visible as the characters that spell it.
fn escaped(text: &str) -> String {
    text.escape_debug().to_string()
}

/// How long ago, in the largest unit that still reads as a count. Absent at
/// either end means NO AGE IS INVENTED: an entry written with no clock, and a
/// reader with no clock of its own, are both unknowable rather than zero.
fn age(recorded: Option<u64>, now: Option<u64>) -> String {
    let (Some(recorded), Some(now)) = (recorded, now) else {
        return UNKNOWN_AGE.to_string();
    };
    ago(now.saturating_sub(recorded))
}

/// A span of seconds as the largest unit that still reads as a count.
///
/// SHARED, because a raw second count is unreadable wherever it appears and
/// this report had two spellings of the same idea: `30814s ago` in the presence
/// line, which no reader converts to eight hours in their head, and this.
///
/// IT STOPS AT HOURS ON PURPOSE. A five-deep ring on a machine used a few times
/// a week holds day-old entries, and `27h ago` says which day-old entry it is
/// where `1d ago` would flatten twenty-seven hours and forty-seven into one
/// answer.
pub fn ago(seconds: u64) -> String {
    match seconds {
        ..60 => format!("{seconds}s ago"),
        60..3_600 => format!("{}m ago", seconds / 60),
        _ => format!("{}h ago", seconds / 3_600),
    }
}

/// An entry this cannot read, QUOTED AND TRUNCATED. The quotes are this arm's
/// own, marking off a fragment of a file from the sentence around it; the
/// escaping inside them is `escaped`, the same rule the readable arm runs.
fn complaint(entry: &str) -> String {
    let held: String = entry.chars().take(QUOTED_MAX).collect();
    // NO INDENT OF ITS OWN. The row's mark supplies it now, and a
    // hand-written pair of spaces here put this line two columns past its
    // readable neighbours.
    format!("unreadable entry: \"{}\"", escaped(&held))
}

/// How much of an unreadable entry is quoted back. Enough to recognize, short
/// enough that a file of garbage cannot fill the report.
const QUOTED_MAX: usize = 60;

/// A reader with no clock of its own, and an entry written without one.
const UNKNOWN_AGE: &str = "age unknown";

/// What an absent log says. THE PARENTHESIS IS THE HONEST HALF: the write is
/// fail-quiet, so nothing here can tell an unused log from one that could not
/// be written, and the line must not claim the first.
const NOTHING_RECORDED: &str = "pns doctor: no decision has been recorded yet \
     (no event has run since this was installed, or none could be written).";
