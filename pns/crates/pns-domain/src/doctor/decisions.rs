use crate::KEPT;
const NO_CLOCK: &str = "-";

/// The decision log as the doctor's own section: a heading and one rendered
/// entry per line, newest first, capped at `KEPT`.
///
/// `contents` is the file, `None` when there is none. `now` is the reader's
/// own clock, which ages the entries and nothing else.
///
/// IT REPORTS HISTORY, NEVER HEALTH. Nothing here reaches an exit code: an
/// empty log on a fresh machine is not a failure, and neither is one this
/// could not parse.
pub fn section(contents: Option<&str>, now: Option<u64>) -> Vec<String> {
    let entries: Vec<&str> = contents
        .unwrap_or_default()
        .lines()
        .filter(|entry| !entry.trim().is_empty())
        .collect();
    if entries.is_empty() {
        return vec![NOTHING_RECORDED.to_string()];
    }
    // NEWEST FIRST, because the ring is written by APPEND and the operator
    // came to look at the card that just did or did not arrive.
    let shown: Vec<&&str> = entries.iter().rev().take(KEPT).collect();
    let mut rendered = vec![heading(shown.len())];
    rendered.extend(shown.into_iter().map(|entry| render(entry, now)));
    rendered
}

/// The section's own first line, counting what it is ABOUT TO SHOW rather than
/// the cap: a heading claiming five over one entry invents four decisions.
fn heading(shown: usize) -> String {
    let counted = if shown == 1 {
        "the last decision,".to_string()
    } else {
        format!("the last {shown} decisions,")
    };
    format!("pns doctor: {counted}{HEADING_TAIL}")
}

/// The rest of every heading. THE actionId IS TOLD HONESTLY rather than
/// printed as an empty field: pns never has one, because moshi mints it inside
/// the approval round trip and answers with an exit code.
const HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
     is recorded: moshi mints it inside the approval round trip and never hands it back.";

/// One entry, as its age and the rest of the line it was written as.
///
/// THE BODY IS NEVER PARSED, only ESCAPED and printed. The whole reader is
/// the one split below, which is what keeps a format change in `line` from
/// needing a matching change here.
fn render(entry: &str, now: Option<u64>) -> String {
    let Some((stamp, rest)) = entry.split_once(' ') else {
        return complaint(entry);
    };
    let recorded = if stamp == NO_CLOCK {
        None
    } else {
        match crate::count::parse_count(stamp) {
            Some(recorded) => Some(recorded),
            // NOT DROPPED SILENTLY: a log that hides the one entry that
            // mattered is worse than one that says it cannot read it.
            None => return complaint(entry),
        }
    };
    format!("  {}: {}", age(recorded, now), escaped(rest))
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
    let seconds = now.saturating_sub(recorded);
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
    format!("  unreadable entry: \"{}\"", escaped(&held))
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
