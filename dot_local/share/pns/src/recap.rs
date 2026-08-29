//! The return recap's body: one window of activity, said in one message.
//!
//! POLICY ONLY, in `missed_notifications`'s style: every function here is a
//! total function of its arguments, with no config, no clock, no environment,
//! no file and no printing. The composition root reads the window off the
//! activity ring, resolves the local wall clock, and posts what comes back.
//!
//! THE COUNT NEVER LIES, which is the rule the whole module is arranged
//! around. The header's count is the length of the window that was READ, and
//! it is composed before anything is cut, so a body that ran out of room still
//! names a total it can back. The budget then cuts LINES and the LENGTH OF A
//! LINE, never a count, and never a line that says something needs the
//! operator.
//!
//! TWO BUDGETS, BOTH ENFORCED. Twenty-five lines is the locked one, and a
//! character ceiling sits beside it because the locked property is ONE Discord
//! message and a line has a length: twenty-five full-width timeline lines
//! MEASURED at 2,859 characters, and the gateway splits at 1,900. `fit` owns
//! both.
//!
//! THE PRIVACY RULE IS THE JOURNAL'S, INHERITED. These lines carry the
//! operator's own text, so nothing here prints: the caller posts the body to
//! the same durable route the live events already reached, and no pns command
//! renders it to a terminal.

use crate::missed_notifications::{Entry, needing_you};

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
/// A HUNDRED CHARACTERS OF HEADROOM is deliberate. The count is characters and
/// the gateway's is bytes, so a body of multi-byte text costs more on the wire
/// than it does here, and the gap is what covers the difference.
pub const MAX_CHARS: usize = 1_800;

/// The local wall clock as a total function this module is HANDED rather than
/// one it reads. `Some(epoch)` becomes `HH:MM`; an unknown epoch becomes a
/// placeholder of the same width, so a line with no clock still lines up under
/// the ones that have one.
pub type Clock<'clock> = &'clock dyn Fn(Option<u64>) -> String;

/// One part of the body, and whether the budget may cut it.
///
/// THE HEADING IS THE FIRST LINE, which is what makes trimming a tail slice:
/// a cut section keeps its heading, as many lines as fit, and one line naming
/// the true remainder.
#[derive(Debug, PartialEq)]
pub struct Section {
    pub lines: Vec<String>,
    /// Whether the budget may cut this section. NEEDS YOU never is, and
    /// neither is a section that is one line long: cutting those buys nothing
    /// and loses the only sentence they had.
    pub trimmable: bool,
}

/// The v3 body, in order: the window header, what needs the operator, the
/// night in order, the two sections nothing sources yet, and the pointer to
/// where the full text lives.
///
/// SECTIONS 4 AND 5 SAY THEY ARE UNCONFIGURED rather than being omitted. A
/// section that vanished would be indistinguishable from a night with no
/// merges and no review findings, which is a different claim entirely.
pub fn sections(entries: &[Entry], from: &str, to: &str, clock: Clock) -> Vec<Section> {
    vec![
        Section {
            lines: vec![header(entries.len(), from, to)],
            trimmable: false,
        },
        needs_you_section(entries),
        night_section(entries, clock),
        Section {
            lines: vec![NEW_BEHAVIOR_UNCONFIGURED.to_string()],
            trimmable: false,
        },
        Section {
            lines: vec![CAUGHT_BY_REVIEW_UNCONFIGURED.to_string()],
            trimmable: false,
        },
        Section {
            lines: vec![TAIL.to_string()],
            trimmable: false,
        },
    ]
}

/// The whole body, fitted to the budget and joined into one message.
pub fn body(entries: &[Entry], from: &str, to: &str, clock: Clock) -> String {
    fit(&sections(entries, from, to, clock), MAX_LINES).join("\n")
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
fn header(counted: usize, from: &str, to: &str) -> String {
    format!(
        "While you were away, {from}-{to} · {}",
        crate::missed_notifications::event_count(counted)
    )
}

/// What is still waiting on the operator, newest first and never cut.
fn needs_you_section(entries: &[Entry]) -> Section {
    let waiting = needing_you(entries);
    let mut lines = vec![NEEDS_YOU_HEADING.to_string()];
    if waiting.is_empty() {
        lines.push(NOTHING_WAITING.to_string());
    } else {
        lines.extend(
            waiting
                .iter()
                .rev()
                .map(|entry| format!("- {}", described(entry))),
        );
    }
    Section {
        lines,
        trimmable: false,
    }
}

/// The night itself, oldest first, one line per event. THE ONLY TRIMMABLE
/// SECTION, because it is the only one whose length follows the window's.
fn night_section(entries: &[Entry], clock: Clock) -> Section {
    let mut lines = vec![NIGHT_HEADING.to_string()];
    if entries.is_empty() {
        // SAID RATHER THAN LEFT BLANK, for `NOTHING_WAITING`'s own reason: a
        // heading with nothing under it reads as a section that broke.
        lines.push(NOTHING_HAPPENED.to_string());
        return Section {
            lines,
            trimmable: false,
        };
    }
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
        trimmable: true,
    }
}

/// One event as a line of the timeline: who, in what state, on what, and what
/// it said.
///
/// NO DANGLING PUNCTUATION for an entry that carries no project or no detail,
/// which is `rendered`'s own rule in the journal: a line ending in a colon
/// reads as truncated rather than complete.
fn described(entry: &Entry) -> String {
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
fn mark(state: &str) -> &'static str {
    if crate::missed_notifications::NEEDS_YOU.contains(&state) {
        // `failed` is in that list AND is its own kind of news, so it is
        // answered before the waiting mark rather than inside it.
        return if state == "failed" { "!" } else { "?" };
    }
    if state == "done" { "+" } else { " " }
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
    let reserved: usize = sections
        .iter()
        .filter(|section| !section.trimmable)
        .map(|section| section.lines.len())
        .sum();
    // THE SEPARATOR COUNTS: `body` joins with newlines, so a line costs its own
    // length plus one. Counting one for the last line too leaves the ceiling a
    // character under rather than a character over.
    let reserved_chars: usize = sections
        .iter()
        .filter(|section| !section.trimmable)
        .map(|section| spent(&section.lines))
        .sum();
    let mut room = budget.saturating_sub(reserved);
    let mut fitted = Vec::new();
    for section in sections {
        if !section.trimmable {
            fitted.extend(section.lines.iter().cloned());
            continue;
        }
        let content = section.lines.len() - 1;
        // TWO LINES ARE THE FLOOR for a section that has to be cut: its own
        // heading, and the line naming what was left out. With less room than
        // that the section says nothing at all, and the header's count is
        // still the whole window.
        let shown = if section.lines.len() <= room {
            content
        } else if room < 2 {
            room = 0;
            continue;
        } else {
            room - 2
        };
        let dropped = content - shown;
        let remainder = (dropped > 0).then(|| format!("...and {dropped} more"));
        // EVERY OTHER LINE IN THE MESSAGE IS ALREADY SPOKEN FOR by the time
        // the share is worked out: the protected sections, this section's own
        // heading, and the remainder line if there is one. What is left is
        // divided evenly, LESS THE NEWLINE each surviving line costs on top of
        // its own text. A line already under its share simply keeps its whole
        // length, which leaves the total under the ceiling rather than on it.
        let spoken_for = reserved_chars
            + spent(&section.lines[..1])
            + remainder.as_deref().map_or(0, |line| spent(&[line]));
        let share = (MAX_CHARS.saturating_sub(spoken_for) / shown.max(1)).saturating_sub(1);
        fitted.push(section.lines[0].clone());
        fitted.extend(
            section.lines[1..=shown]
                .iter()
                .map(|line| crate::render::clipped(line, share)),
        );
        if let Some(remainder) = remainder {
            fitted.push(remainder);
        }
        room = room.saturating_sub(section.lines.len());
    }
    fitted
}

/// What a run of lines costs the character budget: its own text plus the
/// newline that joins it to the next one.
fn spent(lines: &[impl AsRef<str>]) -> usize {
    lines
        .iter()
        .map(|line| line.as_ref().chars().count() + 1)
        .sum()
}

const NEEDS_YOU_HEADING: &str = "NEEDS YOU";

/// Said rather than left blank: an empty section reads as a section that
/// broke, and this one is the reason the message exists.
const NOTHING_WAITING: &str = "- nothing is waiting on you";

const NIGHT_HEADING: &str = "THE NIGHT IN ORDER";

/// An empty window, which the event path never posts (nothing is under every
/// threshold) and a hand-run `pns recap` reaches whenever it is pointed at a
/// quiet stretch.
const NOTHING_HAPPENED: &str = "- nothing was recorded in this window";

/// The two sections whose source is not pns. Named as unconfigured rather than
/// omitted, so a night with no merges reads differently from a pns that was
/// never told where to look for them.
const NEW_BEHAVIOR_UNCONFIGURED: &str =
    "NEW BEHAVIOR: not configured (no merged pull request source).";

const CAUGHT_BY_REVIEW_UNCONFIGURED: &str =
    "CAUGHT BY REVIEW, AND IMPLEMENTED: not configured (no review notes source).";

/// Where the part that did not fit still lives, in full. Every event in the
/// window already reached the durable log when it happened, which is what
/// makes cutting lines here safe at all.
const TAIL: &str = "Every event above is in #pns in full.";

#[cfg(test)]
mod tests {
    use super::{MAX_CHARS, MAX_LINES, Section, body, fit, sections};
    use crate::missed_notifications::Entry;

    /// A fixed clock, so the fixtures state a time rather than reading one.
    fn clock(at: Option<u64>) -> String {
        match at {
            Some(epoch) => format!("{:02}:{:02}", (epoch / 3600) % 24, (epoch / 60) % 60),
            None => "--:--".to_string(),
        }
    }

    /// One event in the window: an epoch, a state, and text naming its place.
    fn acted(at: u64, state: &str, detail: &str) -> Entry {
        Entry {
            at: Some(at),
            agent: "claude".to_string(),
            state: state.to_string(),
            project: "dotfiles".to_string(),
            branch: String::new(),
            detail: detail.to_string(),
        }
    }

    /// A window of `count` finished turns, each naming its own index.
    fn window(count: usize) -> Vec<Entry> {
        (0..count)
            .map(|which| {
                acted(
                    1_756_500_000 + which as u64 * 60,
                    "done",
                    &format!("turn {which}"),
                )
            })
            .collect()
    }

    #[test]
    fn the_body_opens_with_the_window_and_its_count_and_puts_needs_you_above_the_night() {
        // THE FIRST LINE IS THE THREAD'S TITLE, because hermes names a forum
        // thread after it, so the header has to be first and has to read as a
        // title on its own.
        let mut entries = window(3);
        entries.insert(1, acted(1_756_500_030, "blocked", "a decision is waiting"));
        let lines = fit(&sections(&entries, "23:04", "06:15", &clock), MAX_LINES);
        assert_eq!(
            lines[0], "While you were away, 23:04-06:15 · 4 events",
            "{lines:?}"
        );
        let urgent = lines
            .iter()
            .position(|line| line == "NEEDS YOU")
            .expect("a needs-you section");
        let night = lines
            .iter()
            .position(|line| line == "THE NIGHT IN ORDER")
            .expect("a timeline");
        assert!(urgent < night, "the night came first: {lines:?}");
        assert_eq!(
            lines[urgent + 1],
            "- claude/blocked dotfiles: a decision is waiting",
            "{lines:?}"
        );
    }

    #[test]
    fn the_night_is_oldest_first_one_line_per_event_and_marked_by_its_state() {
        // THE MARKERS ARE MECHANICAL, off the state word alone: a finished
        // turn, a turn that died and a turn waiting on the operator are what
        // the eye scans for, and everything else stays unmarked so they stand
        // out.
        let entries = vec![
            acted(1_756_500_000, "done", "the first"),
            acted(1_756_500_060, "failed", "the second"),
            acted(1_756_500_120, "asked", "the third"),
            acted(1_756_500_180, "stale", "the fourth"),
        ];
        let lines = fit(&sections(&entries, "23:04", "06:15", &clock), MAX_LINES);
        let night = lines
            .iter()
            .position(|line| line == "THE NIGHT IN ORDER")
            .expect("a timeline");
        assert_eq!(
            &lines[night + 1..night + 5],
            [
                "20:40 + claude/done dotfiles: the first",
                "20:41 ! claude/failed dotfiles: the second",
                "20:42 ? claude/asked dotfiles: the third",
                "20:43   claude/stale dotfiles: the fourth",
            ],
            "{lines:?}"
        );
    }

    #[test]
    fn the_two_sections_nothing_sources_yet_say_so_rather_than_vanishing() {
        // A SECTION THAT VANISHED would read as a night with no merges and no
        // findings, which is a different claim from "nobody told pns where to
        // look".
        let rendered = body(&window(2), "23:04", "06:15", &clock);
        assert!(
            rendered.contains("NEW BEHAVIOR: not configured"),
            "{rendered}"
        );
        assert!(
            rendered.contains("CAUGHT BY REVIEW, AND IMPLEMENTED: not configured"),
            "{rendered}"
        );
        assert!(
            rendered.ends_with("Every event above is in #pns in full."),
            "{rendered}"
        );
    }

    #[test]
    fn a_window_too_long_for_the_budget_cuts_lines_and_never_a_count_or_a_needs_you() {
        // THE BUDGET IS ENFORCED, NOT HOPED. Eighty events is a real overnight
        // window and a naive body is eighty lines long; what has to survive is
        // every urgent line, the true header count, and a remainder that is the
        // real number left out rather than what happened to fit.
        let mut entries = window(80);
        for (which, at) in [
            (10, 1_756_500_600),
            (40, 1_756_502_400),
            (70, 1_756_504_200),
        ] {
            entries[which] = acted(at, "blocked", &format!("urgent {which}"));
        }
        let lines = fit(&sections(&entries, "23:04", "06:15", &clock), MAX_LINES);

        assert!(
            lines.len() <= MAX_LINES,
            "the budget was exceeded: {} lines",
            lines.len()
        );
        assert!(
            lines[0].ends_with("· 80 events"),
            "the header counts the window, not the survivors: {}",
            lines[0]
        );
        for urgent in ["urgent 10", "urgent 40", "urgent 70"] {
            assert!(
                lines.iter().any(|line| line.contains(urgent)),
                "{urgent} was cut: {lines:?}"
            );
        }
        let remainder = lines
            .iter()
            .find(|line| line.starts_with("...and "))
            .expect("a remainder line");
        let shown = lines
            .iter()
            .filter(|line| {
                line.contains("claude/") && line.starts_with(|first: char| first.is_ascii_digit())
            })
            .count();
        assert_eq!(
            remainder,
            &format!("...and {} more", 80 - shown),
            "the remainder disagreed with what was shown: {lines:?}"
        );
    }

    /// The ring's own field cap, stated here so the fixture below is the
    /// widest line the engine can actually write rather than an invented one.
    const ACTIVITY_MAX_CHARS: usize = 120;

    #[test]
    fn a_worst_case_window_stays_inside_one_discord_message() {
        // ONE MESSAGE, AS LOCKED, and the line budget alone never bought it.
        // MEASURED on the real binary before the character ceiling existed:
        // forty entries at the ring's own field cap rendered as 2,859
        // characters inside 25 lines, and the operator's own hermes adapter
        // splits a Discord message at 1,900. On the plain route that is two
        // messages in the channel; on the forum route the tail lands as
        // follow-ups behind the thread starter. Either way the locked property
        // is gone, and nothing here would have noticed.
        //
        // WORST CASE MEANS EVERY FIELD FULL: forty events, each with the
        // longest detail the writer will store.
        let entries: Vec<Entry> = (0..40)
            .map(|which| {
                acted(
                    1_756_500_000 + which as u64 * 60,
                    "done",
                    &"d".repeat(ACTIVITY_MAX_CHARS),
                )
            })
            .collect();

        let body = body(&entries, "23:04", "06:15", &clock);

        assert!(
            body.chars().count() <= MAX_CHARS,
            "the recap would be split into two Discord messages: {} chars",
            body.chars().count()
        );
        // AND THE COUNTS ARE STILL TRUE, which is what the ceiling may never
        // buy itself: the header names the whole window and the remainder
        // names the whole tail, however short each surviving line was cut.
        assert!(
            body.starts_with("While you were away, 23:04-06:15 · 40 events"),
            "the header's count paid for the ceiling: {body}"
        );
        let shown = body
            .lines()
            .filter(|line| line.contains("claude/done"))
            .count();
        assert!(
            body.contains(&format!("...and {} more", 40 - shown)),
            "the remainder disagrees with the {shown} lines that survived: {body}"
        );
        // AND LINES WERE CUT RATHER THAN DROPPED, which is the direction the
        // ceiling is supposed to fail in: a timeline that says less about more
        // is still the night in order.
        assert!(
            shown > 10,
            "the ceiling was paid for by dropping the night instead: {shown} lines"
        );
    }

    #[test]
    fn a_needs_you_list_longer_than_the_whole_budget_is_still_never_cut() {
        // THE ONE THING ALLOWED PAST THE BUDGET, stated as a test so nobody
        // "fixes" it. A recap that dropped what is waiting on the operator has
        // failed at the one job the phone card could not do for it.
        let entries: Vec<Entry> = (0..40)
            .map(|which| {
                acted(
                    1_756_500_000 + which as u64 * 60,
                    "blocked",
                    &format!("urgent {which}"),
                )
            })
            .collect();
        let lines = fit(&sections(&entries, "23:04", "06:15", &clock), MAX_LINES);
        for which in 0..40 {
            assert!(
                lines
                    .iter()
                    .any(|line| line.contains(&format!("urgent {which}"))),
                "urgent {which} was cut"
            );
        }
        assert!(lines[0].ends_with("· 40 events"), "{}", lines[0]);
    }

    #[test]
    fn a_trimmable_section_with_no_room_left_says_nothing_rather_than_half_a_line() {
        // THE FLOOR IS TWO LINES, its own heading and the remainder. Below that
        // the section is dropped whole and the header's count is still the
        // window's, which is the only number that has to survive.
        let cut = fit(
            &[
                Section {
                    lines: vec!["a header".to_string()],
                    trimmable: false,
                },
                Section {
                    lines: vec!["THE NIGHT IN ORDER".to_string(), "one".to_string()],
                    trimmable: true,
                },
            ],
            1,
        );
        assert_eq!(cut, ["a header"], "{cut:?}");
    }
}
