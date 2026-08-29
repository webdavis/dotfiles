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
/// night in order, the two sections nothing sources yet, and the pointer to
/// where the full text lives.
///
/// SECTIONS 4 AND 5 SAY THEY ARE UNCONFIGURED rather than being omitted. A
/// section that vanished would be indistinguishable from a night with no
/// merges and no review findings, which is a different claim entirely.
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
) -> Vec<Section> {
    let mut parts = vec![
        Section {
            lines: vec![header(entries.len(), from, to)],
            trimmable: false,
        },
        needs_you_section(entries),
        night_section(entries, clock, timeline),
    ];
    parts.extend([
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
    ]);
    parts
}

/// The whole body, fitted to the budget and joined into one message.
pub fn body(entries: &[Entry], from: &str, to: &str, clock: Clock, timeline: Timeline) -> String {
    fit(&sections(entries, from, to, clock, timeline), MAX_LINES).join("\n")
}

/// The window as the summarizer is handed it: what it is looking at, what to
/// do with it, and then the mechanical timeline itself, one event per line.
///
/// THE PROMPT CARRIES THE SAME LINES THE FALLBACK WOULD POST, so the model is
/// asked to compress exactly what pns would otherwise have said, and the two
/// outcomes are comparable. Nothing else about the operator's machine crosses:
/// no transcript, no config, no state beyond the window itself.
///
/// THE WORDING IS NOT THE DEFENCE and is not asked to be. A model that ignores
/// every sentence here still cannot move a count, forge a section or reach the
/// phone, and none of that is because it was asked not to: `Timeline` decides
/// where its answer may land, `answer` decides what a line may contain, and
/// `night_section` prefixes every line it writes and cuts the list to the
/// window's own length, so an answer that reads as a heading renders as content
/// and an answer longer than the night cannot be counted as one.
pub fn prompt(entries: &[Entry], clock: Clock) -> String {
    let mut text = String::from(INSTRUCTION);
    for line in night_section(entries, clock, Timeline::Mechanical)
        .lines
        .iter()
        .skip(1)
    {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// What a summarizer said, made into timeline lines, or None for every way of
/// saying nothing usable: too much, repaired on the way in, or nothing at all.
///
/// EVERY LINE IS FLATTENED AND CAPPED, which is what stops an answer forging a
/// section heading with a newline of its own or reaching Discord with the
/// control bytes a bare `ollama run` interleaves into its output. A model's
/// answer is somebody else's text arriving in a message pns signs its name to,
/// and it is treated as exactly that.
///
/// THE BYTE CAP IS THE ONE GUARD THE SEAM DID NOT ALREADY HAVE. `run_bounded`
/// is bounded in TIME and not in bytes, and this is its first caller fed a
/// model: a backend that streams for as long as the deadline allows would
/// otherwise hand a message of any size to the composition below. ACCEPTED
/// LIMIT: the answer has already been read into memory by the time it gets
/// here, so this bounds what is POSTED rather than what was read. The READ is
/// bounded a byte further out, at `system::run_bounded`'s own cap, so an answer
/// that arrives here over the cap is one the seam stopped rather than one it
/// buffered whole.
///
/// AND A REPAIRED ANSWER IS REFUSED WHOLESALE, matching
/// `system::parse_idle_nanoseconds` on the same seam's output: the runner reads
/// lossily, so a replacement character means invalid bytes were substituted
/// somewhere in the answer, and a timeline is not more trustworthy than an idle
/// counter. The plain list is the better message either way.
pub fn answer(raw: &str) -> Option<Vec<String>> {
    if raw.len() > MAX_ANSWER_BYTES || raw.contains('\u{FFFD}') {
        return None;
    }
    let lines: Vec<String> = raw
        .lines()
        .map(|line| safe_line(line, SUMMARIZED_MAX_CHARS))
        .filter(|line| !line.is_empty())
        .collect();
    (!lines.is_empty()).then_some(lines)
}

/// One line of somebody else's answer, made safe to put in the message: every
/// kind of whitespace becomes one space, every control byte and every INVISIBLE
/// character is dropped whole, and what is left is capped to a timeline line's
/// width.
///
/// THE FORMAT CHARACTERS GO TOO, and `char::is_control` does not reach them:
/// U+202E RIGHT-TO-LEFT OVERRIDE and U+200B ZERO WIDTH SPACE are Unicode
/// category Cf, which is neither control nor whitespace, and Discord honours
/// the override by rendering a line in an order nobody wrote it in. The ranges
/// below are that category's bidi, zero-width, invisible-operator and
/// byte-order marks; anything a reader cannot see has no business in a line
/// pns signs its name to.
///
/// DROPPED RATHER THAN ESCAPED, which is the opposite of the decision log's
/// rule and for the opposite reason: that one is read on a terminal by an
/// operator asking what happened, so an escape is evidence, while this is a
/// sentence posted to a chat channel, where `\u{1b}` in the middle of a line is
/// only noise.
///
/// AND THE HEAD IS WHAT SURVIVES THE WIDTH, which is why the cut is `clipped`
/// and not the flatten's own cap. `flatten_reply` keeps a TURN's tail, because
/// a turn states its conclusion at the end; this is a line somebody composed,
/// whose beginning names what it is about, and `fit` goes on to cut the same
/// line from the same end. Cutting the two ends in turn would leave the middle
/// of a sentence and nothing to say which part of it that was.
fn safe_line(line: &str, max_chars: usize) -> String {
    let printable: String = line
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .filter(|character| !character.is_control() && !is_invisible(*character))
        .collect();
    crate::render::clipped(
        &crate::render::flatten_reply(&printable, usize::MAX),
        max_chars,
    )
}

/// Whether a character is one the reader cannot see: the Unicode FORMAT
/// ranges, stated as ranges because std has no category lookup and this crate
/// takes no dependency for one.
fn is_invisible(character: char) -> bool {
    matches!(
        character,
        '\u{00ad}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
    )
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
fn night_section(entries: &[Entry], clock: Clock, timeline: Timeline) -> Section {
    if entries.is_empty() {
        // SAID RATHER THAN LEFT BLANK, for `NOTHING_WAITING`'s own reason: a
        // heading with nothing under it reads as a section that broke. IT
        // ANSWERS EVERY VARIANT, because a window with no events has no night
        // for anybody to have summarized.
        return Section {
            lines: vec![NIGHT_HEADING.to_string(), NOTHING_HAPPENED.to_string()],
            trimmable: false,
        };
    }
    if let Timeline::Summarized(lines) = timeline {
        let mut summarized = vec![NIGHT_HEADING.to_string()];
        summarized.extend(
            lines
                .iter()
                .take(entries.len())
                .map(|line| format!("- {line}")),
        );
        return Section {
            lines: summarized,
            trimmable: true,
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

/// What the operator is told when a configured summarizer produced nothing:
/// the same plain list an unconfigured machine posts, and one line saying which
/// of the two this is.
///
/// ONE SENTENCE FOR EVERY WAY OF FAILING, deliberately. A spawn that found no
/// such command, a non-zero exit, an empty answer, a deadline and an answer the
/// cap or the lossy read refused are one outcome to the reader of a recap: the
/// model did not help with this one. Which of them it was is the operator's to
/// find by running the command themselves, and naming it here would put a
/// diagnostic in a message whose whole job is the night.
///
/// AN ABSENT KEY IS NOT ON THAT LIST, because it is not a failure. A machine
/// with no summarizer configured posts the plain lists and says nothing, which
/// is the working setting and the common one; this sentence exists to tell the
/// two apart, so saying it on both would be saying nothing.
///
/// IT IS SAID IN THE NIGHT'S OWN HEADING, which is why it names no direction:
/// it sits above the list it describes and is dropped with it.
const SUMMARIZER_SILENT: &str = "(The summarizer did not answer, so this is the plain list.)";

/// How much of a summarizer's answer is worth posting at all.
///
/// ONE BYTE MORE THAN THIS IS WHAT THE SUMMARIZER'S READ IS CAPPED AT, so an
/// answer that reaches this function over the cap is one the seam STOPPED
/// rather than one it buffered whole, and the one extra byte is what keeps
/// over-cap distinguishable from exactly-at-cap.
///
/// SIXTEEN KIBIBYTES is far past any honest answer to the prompt (the budget
/// itself is twenty-five lines and the message ceiling is 1,800 characters, so
/// a full one is around three) and far under what a backend that ignored the
/// instruction and narrated its own reasoning would produce. Past this the
/// answer is not a timeline, whatever it is, so the plain list is the better
/// message.
pub const MAX_ANSWER_BYTES: usize = 16 * 1024;

/// How wide one line of a summarizer's answer may be.
///
/// THE RING'S OWN FIELD CAP, stated rather than imported because the constant
/// lives with the writer in the binary: a summarized line stands where a
/// mechanical one would have, so it is held to the same width and the character
/// budget behaves the same either way.
const SUMMARIZED_MAX_CHARS: usize = 120;

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

/// What the summarizer is asked to do, ahead of the window itself.
///
/// SELECT AND COMPRESS, NEVER INVENT, said plainly because a backend that
/// follows instructions writes a better recap for it. It is not what makes the
/// rule hold: see `prompt`.
const INSTRUCTION: &str = "Below are the events of one stretch of unattended work on one \
     machine, oldest first, one line per event.\n\n\
     Rewrite them as the timeline somebody reads when they come back to the desk: one line \
     per thing that happened, at most 20 lines, keeping the times and the project names. \
     Put the runs that are one story on one line, and leave out what nobody would ask \
     about. Select and compress only: never state anything that is not below, and never \
     count anything. Answer with the timeline lines alone, no heading, no numbering and no \
     commentary.\n\n";

#[cfg(test)]
mod tests {
    use super::{
        INSTRUCTION, MAX_ANSWER_BYTES, MAX_CHARS, MAX_LINES, SUMMARIZED_MAX_CHARS,
        SUMMARIZER_SILENT, Section, Timeline, answer, body, fit, prompt, sections,
    };
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
        let lines = fit(
            &sections(&entries, "23:04", "06:15", &clock, Timeline::Mechanical),
            MAX_LINES,
        );
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
        let lines = fit(
            &sections(&entries, "23:04", "06:15", &clock, Timeline::Mechanical),
            MAX_LINES,
        );
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
        let rendered = body(&window(2), "23:04", "06:15", &clock, Timeline::Mechanical);
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
        let lines = fit(
            &sections(&entries, "23:04", "06:15", &clock, Timeline::Mechanical),
            MAX_LINES,
        );

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

        let body = body(&entries, "23:04", "06:15", &clock, Timeline::Mechanical);

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
        let lines = fit(
            &sections(&entries, "23:04", "06:15", &clock, Timeline::Mechanical),
            MAX_LINES,
        );
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

    // --- what a summarizer is allowed to say ---------------------------------

    #[test]
    fn a_summarizers_line_cannot_carry_a_break_or_a_control_byte_into_the_message() {
        // SOMEBODY ELSE'S TEXT IN A MESSAGE PNS SIGNS. A newline inside an
        // answer forges a section heading; an ESC reaches Discord verbatim,
        // which a bare `ollama run` interleaves into its own output as a matter
        // of course. Both are answered where the answer becomes lines, so no
        // caller has to remember.
        // THE RUNS AND THE ENDS GO TOO, which is the flatten's own half of the
        // job: a line arriving with a leading tab and a double space renders
        // with both, and a timeline of lines that do not start in the same
        // column is a timeline nobody scans.
        let lines = answer("  one\u{1b}[2K \t two\n\nthree\u{0}four\rfive  \n").expect("an answer");
        assert_eq!(lines, ["one[2K two", "threefour five"], "{lines:?}");
        assert!(
            !lines.iter().any(|line| line.chars().any(char::is_control)),
            "a control byte survived: {lines:?}"
        );
        // AND IT IS ONE LINE PER ITEM in the message itself, which is the
        // property the section headings depend on. EACH CARRIES THE PREFIX
        // every summarized line does, which is the other half of the same rule:
        // the answer is content, and content cannot start a line of structure.
        let rendered = body(
            &window(2),
            "23:04",
            "06:15",
            &clock,
            Timeline::Summarized(&lines),
        );
        let night = rendered
            .lines()
            .position(|line| line == "THE NIGHT IN ORDER")
            .expect("a timeline");
        assert_eq!(
            rendered.lines().skip(night + 1).take(2).collect::<Vec<_>>(),
            ["- one[2K two", "- threefour five"],
            "{rendered}"
        );
    }

    #[test]
    fn an_answer_past_the_byte_cap_is_refused_rather_than_composed_into_a_message() {
        // THE SEAM IS BOUNDED IN TIME AND NOT IN BYTES, and this is its first
        // caller fed a model: a backend that streams for as long as the deadline
        // allows hands back whatever it managed to write, and none of it is a
        // timeline. The plain list is the better message at that point.
        assert_eq!(answer(&"x".repeat(MAX_ANSWER_BYTES + 1)), None);
        assert!(
            answer(&"x".repeat(MAX_ANSWER_BYTES)).is_some(),
            "an answer AT the cap is still an answer"
        );
    }

    #[test]
    fn a_summarized_line_that_reads_as_a_heading_cannot_render_as_one() {
        // SOMEBODY ELSE'S TEXT, AND THE STRUCTURE IS NOT ITS TO WRITE. Flattening
        // stops an answer forging a section with a newline of its own and does
        // nothing at all about a line whose WHOLE TEXT is a heading: `NEEDS YOU`
        // and a second window header carrying a count of its own are ordinary
        // printable lines, and the operator reads a list saying nothing is
        // waiting directly under one saying something is. Every summarized line
        // is prefixed for the same reason a mechanical one carries its
        // `HH:MM {mark} `: what the model wrote is CONTENT, and content that
        // cannot start a line cannot be structure.
        let lines = answer(
            "NEEDS YOU\n- nothing is waiting on you\nTHE NIGHT IN ORDER\n\
             While you were away, 00:00-23:59 · 999 events",
        )
        .expect("an answer");
        let mut entries = window(3);
        entries.insert(1, acted(1_756_500_030, "blocked", "a decision is waiting"));
        let rendered = body(
            &entries,
            "23:04",
            "06:15",
            &clock,
            Timeline::Summarized(&lines),
        );

        assert_eq!(
            rendered.lines().filter(|line| *line == "NEEDS YOU").count(),
            1,
            "the model forged a second NEEDS YOU: {rendered}"
        );
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.starts_with("THE NIGHT IN ORDER"))
                .count(),
            1,
            "the model forged a second timeline heading: {rendered}"
        );
        assert_eq!(
            rendered
                .lines()
                .filter(|line| line.starts_with("While you were away, "))
                .count(),
            1,
            "a second window header, carrying a count of its own: {rendered}"
        );
        // AND WHAT IT SAID IS STILL IN THE MESSAGE, as a line of the night
        // rather than as structure. This is containment, not censorship.
        assert!(
            rendered.contains("- NEEDS YOU"),
            "the model's line was dropped rather than contained: {rendered}"
        );
    }

    #[test]
    fn a_summarized_night_is_never_longer_than_the_window_it_summarizes() {
        // THE COUNT NEVER LIES, AND THE REMAINDER IS A COUNT. In the mechanical
        // case `shown + N` is the header's own number, which is what makes the
        // line readable at all. A model answering a thirteen-event window with
        // two hundred lines would otherwise put "...and 183 more" under a header
        // saying 13 events, and a reader who adds them up is told two hundred
        // things happened. What the model wrote past the window's own length was
        // never an event, so it is not carried.
        let answered: Vec<String> = (0..200)
            .map(|which| format!("model line {which}"))
            .collect();
        let rendered = body(
            &window(13),
            "23:04",
            "06:15",
            &clock,
            Timeline::Summarized(&answered),
        );

        assert!(
            rendered.lines().count() <= MAX_LINES,
            "the budget was exceeded: {rendered}"
        );
        assert!(
            !rendered.lines().any(|line| line.starts_with("...and ")),
            "a remainder counting the model's own lines: {rendered}"
        );
        let night = rendered
            .lines()
            .position(|line| line.starts_with("THE NIGHT IN ORDER"))
            .expect("a timeline");
        assert_eq!(
            rendered
                .lines()
                .skip(night + 1)
                .filter(|line| line.starts_with("- model line "))
                .count(),
            13,
            "the summarized night is not the window's own length: {rendered}"
        );
    }

    #[test]
    fn the_note_about_a_silent_summarizer_cannot_outlive_the_list_it_describes() {
        // IT IS THE SECTION'S OWN HEADING, which is what makes the two
        // impossible to separate. As a protected section of its own it survives
        // a night the budget dropped WHOLE, and then the only list above it is
        // NEEDS YOU: the message says the plain list is plain about a night it
        // does not carry at all.
        let entries: Vec<Entry> = (0..40)
            .map(|which| {
                acted(
                    1_756_500_000 + which as u64 * 60,
                    "blocked",
                    &format!("urgent {which}"),
                )
            })
            .collect();
        let dropped = body(&entries, "23:04", "06:15", &clock, Timeline::Unanswered);
        assert!(
            !dropped.contains("THE NIGHT IN ORDER"),
            "the fixture no longer drops the night whole: {dropped}"
        );
        assert!(
            !dropped.contains(SUMMARIZER_SILENT),
            "a note about a list the message does not carry: {dropped}"
        );

        // AND IT IS STILL SAID WHEN THE LIST IS THERE, which is the whole
        // reason for saying it: the plain list of a night nobody was asked to
        // summarize and the plain list of a model that went quiet read
        // identically otherwise.
        let kept = body(&window(3), "23:04", "06:15", &clock, Timeline::Unanswered);
        assert!(
            kept.contains(SUMMARIZER_SILENT),
            "the fallback stopped saying which of the two lists it is: {kept}"
        );
        let unconfigured = body(&window(3), "23:04", "06:15", &clock, Timeline::Mechanical);
        assert!(
            !unconfigured.contains(SUMMARIZER_SILENT),
            "a machine with no summarizer was told one went quiet: {unconfigured}"
        );
    }

    #[test]
    fn a_summarizers_line_cannot_carry_an_invisible_or_a_reordering_character() {
        // CONTROL BYTES ARE NOT THE WHOLE OF SOMEBODY ELSE'S TEXT. A RIGHT TO
        // LEFT OVERRIDE and a ZERO WIDTH SPACE are Unicode FORMAT characters:
        // neither is `char::is_control` nor `char::is_whitespace`, so both used
        // to pass through, and Discord honours the override by displaying a
        // line in an order nobody wrote it in.
        let lines =
            answer("start\u{202e}desrever\u{200b}end\u{feff}\u{2066}here").expect("an answer");
        assert_eq!(lines, ["startdesreverendhere"], "{lines:?}");
    }

    #[test]
    fn an_answer_the_runner_had_to_repair_is_refused_rather_than_posted() {
        // THE SEAM READS LOSSILY, so invalid bytes arrive here as replacement
        // characters. `parse_idle_nanoseconds` reads one out of the SAME seam as
        // proof the reading is corrupt and refuses the whole thing; a timeline
        // is not more trustworthy than an idle counter, and the plain list is
        // the better message either way.
        assert_eq!(answer("a\u{FFFD}\u{FFFD}b"), None);
    }

    #[test]
    fn the_prompt_asks_for_the_timeline_and_carries_the_window_itself() {
        // WHAT THE MODEL IS ACTUALLY HANDED, pinned where it is composed. Every
        // other test here drives an answer, so a `prompt` gutted to an empty
        // string leaves them all green while a real backend is asked to
        // summarize nothing at all.
        let asked = prompt(&window(3), &clock);
        assert!(
            asked.starts_with(INSTRUCTION),
            "the instruction is not what the model reads first: {asked:?}"
        );
        assert_eq!(
            asked
                .strip_prefix(INSTRUCTION)
                .expect("the instruction")
                .lines()
                .collect::<Vec<_>>(),
            [
                "20:40 + claude/done dotfiles: turn 0",
                "20:41 + claude/done dotfiles: turn 1",
                "20:42 + claude/done dotfiles: turn 2",
            ],
            "the window's own lines are not what follows it: {asked:?}"
        );
        // AND THE HEADING IS NOT IN IT: the model is handed the events, never
        // the structure it is being told not to write.
        assert!(
            !asked.contains("THE NIGHT IN ORDER"),
            "the model was shown the heading it must not repeat: {asked:?}"
        );
    }

    #[test]
    fn a_summarizers_line_is_held_to_a_timeline_lines_width() {
        // A SUMMARIZED LINE STANDS WHERE A MECHANICAL ONE WOULD, so it is held
        // to the same width: the character budget is worked out against lines
        // of that size, and one paragraph-long line would spend the whole
        // message on itself.
        let lines = answer(&"w".repeat(SUMMARIZED_MAX_CHARS + 40)).expect("an answer");
        assert_eq!(lines[0].chars().count(), SUMMARIZED_MAX_CHARS, "{lines:?}");
    }
}
