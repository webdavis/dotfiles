//! The recap, pinned: answers.

#![allow(unused_imports)]

use super::fixtures::*;
use crate::missed::Entry;
use crate::recap::budget::{MAX_CHARS, MAX_LINES, Trim, fit};
use crate::recap::external::{
    EXTERNAL_MAX_CHARS, EXTERNAL_TEXT_CHARS, External, Externals, Found, Sourced, merged, noted,
};
use crate::recap::prompt::{
    INSTRUCTION, MAX_ANSWER_BYTES, SUMMARIZED_MAX_CHARS, SUMMARIZER_SILENT, answer, note_prompt,
    prompt,
};
use crate::recap::sanitize::is_invisible;
use crate::recap::sections::Section;
use crate::recap::sections::{Timeline, body, sections};

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
        &Externals::default(),
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
        &Externals::default(),
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
        &Externals::default(),
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
    let dropped = body(
        &entries,
        "23:04",
        "06:15",
        &clock,
        Timeline::Unanswered,
        &Externals::default(),
    );
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
    let kept = body(
        &window(3),
        "23:04",
        "06:15",
        &clock,
        Timeline::Unanswered,
        &Externals::default(),
    );
    assert!(
        kept.contains(SUMMARIZER_SILENT),
        "the fallback stopped saying which of the two lists it is: {kept}"
    );
    let unconfigured = body(
        &window(3),
        "23:04",
        "06:15",
        &clock,
        Timeline::Mechanical,
        &Externals::default(),
    );
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
    let lines = answer("start\u{202e}desrever\u{200b}end\u{feff}\u{2066}here").expect("an answer");
    assert_eq!(lines, ["startdesreverendhere"], "{lines:?}");
}

#[test]
fn the_arabic_letter_mark_is_stripped_like_every_other_format_character() {
    // SOL 2: U+061C is Unicode category Cf, exactly like the bidi and
    // zero-width characters above it, and was absent from `is_invisible`
    // despite the doc comment's claim to strip the whole category. Its
    // own case, separate from the mixed string above it, so a failure
    // here names the character rather than getting lost among four
    // others.
    assert!(
        is_invisible('\u{061c}'),
        "U+061C is Cf, the category this strips"
    );
    let lines = answer("left\u{061c}right").expect("an answer");
    assert_eq!(lines, ["leftright"], "{lines:?}");
}

#[test]
fn is_invisible_agrees_with_unicode_17_0_across_every_code_point() {
    // A DATA-DRIVEN CHECK, independent of `is_invisible`'s own ranges.
    // The Arabic letter mark case above pins one character the previous
    // transcription missed; a review found the miss went deeper, two of
    // the doc comment's 21 Cf ranges were wrong (one absent, one
    // truncated), nine code points short of the category the comment
    // claims to cover in full. A table that reused `is_invisible`'s own
    // ranges would have missed the same nine, so this one is copied
    // straight from the standard's own Cf listing instead, fetched from
    // https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt
    // on 2026-09-02, and every valid `char` is checked against it.
    const CF_RANGES: &[(u32, u32)] = &[
        (0x00AD, 0x00AD),
        (0x0600, 0x0605),
        (0x061C, 0x061C),
        (0x06DD, 0x06DD),
        (0x070F, 0x070F),
        (0x0890, 0x0891),
        (0x08E2, 0x08E2),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x2064),
        (0x2066, 0x206F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x110CD, 0x110CD),
        (0x13430, 0x1343F),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0001, 0xE0001),
        (0xE0020, 0xE007F),
    ];

    let total_cf_code_points: u32 = CF_RANGES.iter().map(|(lo, hi)| hi - lo + 1).sum();
    assert_eq!(
        total_cf_code_points, 170,
        "the fixture itself should name all 170 Cf code points in Unicode 17.0"
    );

    for codepoint in 0u32..=0x10FFFF {
        let Some(character) = char::from_u32(codepoint) else {
            continue;
        };
        let should_be_invisible = CF_RANGES
            .iter()
            .any(|(lo, hi)| (*lo..=*hi).contains(&codepoint));
        assert_eq!(
            is_invisible(character),
            should_be_invisible,
            "U+{codepoint:04X} disagrees with the standard's own Cf category"
        );
    }
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
