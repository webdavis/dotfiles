//! What a summarizer is asked, and what is taken from its answer.

use super::activity::Event;
use super::budget::Clock;
use super::external::Sourced;
use super::night::{described, mark};
use super::sanitize::safe_line;

/// The review notes as the summarizer is handed them. THE ONLY SECTION WHOSE
/// SOURCE IS UNREADABLE WITHOUT A MODEL: a note is a report somebody wrote for
/// a person, so a mechanical line can name it and nothing more.
pub fn note_prompt(sources: &[Sourced]) -> String {
    external_prompt(NOTE_INSTRUCTION, sources)
}
pub(super) fn external_prompt(instruction: &str, sources: &[Sourced]) -> String {
    let mut text = String::from(instruction);
    for source in sources {
        text.push_str(&source.cite);
        text.push(' ');
        text.push_str(&source.source);
        text.push('\n');
    }
    text
}
/// The four sentences one list section says about itself, held together so
/// that a section cannot be given another section's words.
///
/// BUILT FROM THE SECTION'S OWN CONFIG KEY rather than written out per
/// section, because the sections are now whatever `[recap.sources]` names.
/// One function means the heading a reader sees, the key they wrote and the
/// field in the document cannot drift apart.
pub(super) struct Voice {
    pub(super) heading: String,
    pub(super) unconfigured: String,
    pub(super) unavailable: String,
    pub(super) nothing: String,
}

/// The four sentences for one section name.
pub(super) fn voice(name: &str) -> Voice {
    let heading = name.replace('_', " ").to_uppercase();
    Voice {
        unconfigured: format!("{heading}: not configured."),
        unavailable: format!("{heading}: unavailable (the command could not be run)."),
        nothing: format!("{heading}: nothing in this window."),
        heading,
    }
}

/// What the summarizer is asked of the review notes. `INSTRUCTION`'s rules
/// hold: the wording is not the defence, and the receipts check is.
pub(super) const NOTE_INSTRUCTION: &str = "Below are the review notes written while nobody was watching, \
     each behind the name of the file it came from.\n\n\
     Rewrite them as the list somebody reads to find out what review caught and what was then \
     implemented: one line each, at most 4 lines, saying what was found and what was done \
     about it. START EVERY LINE WITH THE FILE NAME IT CAME FROM, exactly as written below. \
     Select and compress only: never state anything that is not below, and never count \
     anything. Answer with the lines alone, no heading, no numbering and no commentary.\n\n";
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
pub fn prompt(events: &[Event], clock: Clock) -> String {
    let mut text = String::from(INSTRUCTION);
    for event in events {
        text.push_str(&format!(
            "{} {} {}\n",
            clock(Some(event.at)),
            mark(&event.state),
            described(event)
        ));
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
/// THE BYTE CAP HERE IS THE NARROWER OF TWO. `system::run_bounded` bounds the
/// READ, in time and in bytes both, and refuses anything past its own ceiling;
/// this is the smaller cap on what may be POSTED, and this is the seam's first
/// caller fed a model, which is the child that makes the difference matter. The
/// answer has already been read into memory by the time it reaches here, so an
/// answer that arrives over THIS cap is one the reader let through and this
/// refuses; one over the READER's ceiling never arrives at all.
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
/// What the operator is told when a configured summarizer produced nothing:
/// the same plain list an unconfigured machine posts, and one line saying which
/// of the two this is.
///
/// ONE SENTENCE FOR EVERY WAY OF FAILING, deliberately. A spawn that found no
/// such command, a non-zero exit, an empty answer, a deadline, an answer the
/// cap or the lossy read refused, and an answer whose every line was dropped
/// for naming no source pns fetched are one outcome to the reader of a recap:
/// the model did not help with this one. Which of them it was is the operator's
/// to find by running the command themselves, and naming it here would put a
/// diagnostic in a message whose whole job is the night.
///
/// AN ABSENT KEY IS NOT ON THAT LIST, because it is not a failure. A machine
/// with no summarizer configured posts the plain lists and says nothing, which
/// is the working setting and the common one; this sentence exists to tell the
/// two apart, so saying it on both would be saying nothing.
///
/// IT IS SAID IN THE NIGHT'S OWN HEADING, which is why it names no direction:
/// it sits above the list it describes and is dropped with it.
pub(super) const SUMMARIZER_SILENT: &str =
    "(The summarizer did not answer, so this is the plain list.)";
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
pub(super) const SUMMARIZED_MAX_CHARS: usize = 120;
/// What the summarizer is asked to do, ahead of the window itself.
///
/// SELECT AND COMPRESS, NEVER INVENT, said plainly because a backend that
/// follows instructions writes a better recap for it. It is not what makes the
/// rule hold: see `prompt`.
pub(super) const INSTRUCTION: &str = "Below are the events of one stretch of unattended work on one \
     machine, oldest first, one line per event.\n\n\
     Rewrite them as the timeline somebody reads when they come back to the desk: one line \
     per thing that happened, at most 20 lines, keeping the times and the project names. \
     Put the runs that are one story on one line, and leave out what nobody would ask \
     about. Select and compress only: never state anything that is not below, and never \
     count anything. Answer with the timeline lines alone, no heading, no numbering and no \
     commentary.\n\n";
/// What the summarizer is asked of the whole recap, ahead of the document
/// itself.
///
/// ONE PARAGRAPH OVER THE MECHANICAL DOCUMENT, which is the section's whole
/// job: the counts, the rows and the open list stay exact underneath it, so
/// the model is asked to say what matters rather than to restate them.
///
/// `open` IS NAMED AS OFF LIMITS, and, as `INSTRUCTION` says of its own
/// wording, that sentence is not the defence. The summary is composed into a
/// section of its own and `open` is built from the store and the source
/// commands whatever the model answers, so an answer that rewrote it would
/// render as part of the paragraph and move nothing.
pub const SUMMARY_INSTRUCTION: &str = "Below is one machine's recap of a window of work, as a JSON \
     document.\n\n\
     Write ONE short paragraph, at most 5 sentences, saying what moved, what is waiting, and what \
     the operator should look at first. Select and compress only: never state anything that is not \
     below, never count anything, and never restate the `open` list, which the reader is shown in \
     full underneath you. Answer with the paragraph alone, no heading and no commentary.\n\n";

/// The summary prompt: the instruction, then the document it is about.
///
/// THE INSTRUCTION IS A PARAMETER because `[recap.summarizer] prompt` and
/// `prompt_file` replace it. pns still appends the document, so the operator
/// writes the instruction and never the plumbing.
pub fn summary_prompt(instruction: &str, document: &str) -> String {
    format!("{instruction}{document}\n")
}

/// What one summary answer becomes: its lines, flattened and capped, or None
/// for every way of saying nothing usable.
///
/// THE SAME TREATMENT A TIMELINE LINE GETS, for the same reason: this is text
/// pns did not write, arriving in a message pns signs its name to and in a
/// document another tool parses. THE WIDTH IS THE PARAGRAPH'S OWN, because a
/// summary is prose rather than a row, and the timeline's row width would cut
/// an honest answer to its first sentence.
pub fn summary_answer(raw: &str) -> Option<Vec<String>> {
    if raw.len() > MAX_ANSWER_BYTES || raw.contains('\u{FFFD}') {
        return None;
    }
    let lines: Vec<String> = raw
        .lines()
        .map(|line| safe_line(line, SUMMARY_MAX_CHARS))
        .filter(|line| !line.is_empty())
        .collect();
    (!lines.is_empty()).then_some(lines)
}

/// How wide one line of a summary may be. FIVE SENTENCES OF PROSE, which is
/// what the instruction asks for and roughly four times a timeline row.
const SUMMARY_MAX_CHARS: usize = 480;
