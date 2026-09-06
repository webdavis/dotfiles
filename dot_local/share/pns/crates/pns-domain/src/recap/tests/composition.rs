//! The recap, pinned: composition.

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

#[test]
fn the_body_opens_with_the_window_and_its_count_and_puts_needs_you_above_the_night() {
    // THE FIRST LINE IS THE THREAD'S TITLE, because hermes names a forum
    // thread after it, so the header has to be first and has to read as a
    // title on its own.
    let mut entries = window(3);
    entries.insert(1, acted(1_756_500_030, "blocked", "a decision is waiting"));
    let lines = fit(
        &sections(
            &entries,
            "23:04",
            "06:15",
            &clock,
            Timeline::Mechanical,
            &Externals::default(),
        ),
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
        &sections(
            &entries,
            "23:04",
            "06:15",
            &clock,
            Timeline::Mechanical,
            &Externals::default(),
        ),
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
    let rendered = body(
        &window(2),
        "23:04",
        "06:15",
        &clock,
        Timeline::Mechanical,
        &Externals::default(),
    );
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
        &sections(
            &entries,
            "23:04",
            "06:15",
            &clock,
            Timeline::Mechanical,
            &Externals::default(),
        ),
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

    let body = body(
        &entries,
        "23:04",
        "06:15",
        &clock,
        Timeline::Mechanical,
        &Externals::default(),
    );

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

    // AND THE SAME WINDOW WITH BOTH EXTERNAL SECTIONS AT THEIR OWN WORST
    // CASE, because those two are PROTECTED: every character they spend is
    // reserved before the night is given a share, so their width and their
    // line count are what decide whether one message is still one message.
    // Ten sources each, so both sections are full AND both carry a
    // remainder line.
    let merges: Vec<Sourced> = (0..10)
        .map(|which| merged(200 + which, &"m".repeat(200), ""))
        .collect();
    let notes: Vec<Sourced> = (0..10)
        .map(|which| {
            noted(
                &format!("note-{which}.md"),
                &format!("# {}", "n".repeat(200)),
            )
        })
        .collect();
    // Named through `super`, because the binding above has taken the name.
    let sourced = crate::recap::sections::body(
        &entries,
        "23:04",
        "06:15",
        &clock,
        Timeline::Mechanical,
        &Externals {
            merges: External {
                found: Found::Read(&merges),
                answered: None,
                truncated: false,
            },
            notes: External {
                found: Found::Read(&notes),
                answered: None,
                truncated: false,
            },
        },
    );
    assert!(
        sourced.chars().count() <= MAX_CHARS,
        "two full external sections split the message in two: {} chars",
        sourced.chars().count()
    );
    assert!(
        sourced.starts_with("While you were away, 23:04-06:15 · 40 events"),
        "the header's count paid for the ceiling: {sourced}"
    );
    // AND THE NIGHT SURVIVED THEM, which is the direction that matters: the
    // two protected sections take their reservation off the LENGTH of a
    // timeline line, never off the timeline itself.
    assert!(
        sourced
            .lines()
            .filter(|line| line.contains("claude/done"))
            .count()
            >= 5,
        "the external sections were paid for by dropping the night: {sourced}"
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
        &sections(
            &entries,
            "23:04",
            "06:15",
            &clock,
            Timeline::Mechanical,
            &Externals::default(),
        ),
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
                trim: Trim::Never,
                omitted: 0,
                at_least: false,
            },
            Section {
                lines: vec!["THE NIGHT IN ORDER".to_string(), "one".to_string()],
                trim: Trim::Always,
                omitted: 0,
                at_least: false,
            },
        ],
        1,
    );
    assert_eq!(cut, ["a header"], "{cut:?}");
}
