//! The recap, pinned: external.

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

// --- the two sections whose source is not pns ----------------------------

/// The section under one heading, heading line included, out of a whole
/// body. Read back off the rendered message rather than off the builder, so
/// every assertion below is about what actually reaches Discord.
fn under(rendered: &str, heading: &str) -> Vec<String> {
    rendered
        .lines()
        .skip_while(|line| !line.starts_with(heading))
        .take_while(|line| {
            line.starts_with(heading) || line.starts_with("- ") || line.starts_with("...and")
        })
        .map(str::to_string)
        .collect()
}

/// A recap over one quiet window, with whatever the two external sections
/// were handed.
fn rendered(externals: &Externals) -> String {
    body(
        &window(2),
        "23:04",
        "06:15",
        &clock,
        Timeline::Mechanical,
        externals,
    )
}

/// Only the merges configured, with an optional summarizer answer over
/// them.
fn merges<'sources>(
    sources: &'sources [Sourced],
    answered: Option<&'sources [String]>,
) -> Externals<'sources> {
    Externals {
        merges: External {
            found: Found::Read(sources),
            answered,
            truncated: false,
        },
        ..Externals::default()
    }
}

#[test]
fn every_merge_in_the_window_is_one_cited_line_under_new_behavior() {
    // THE SECTION SAYS WHAT SHIPPED, one line per merged pull request, and
    // every line CARRIES ITS OWN RECEIPT: the number is what an operator
    // follows to the body this line was written from.
    let sources = [
        merged(
            213,
            "feat(pns): a title",
            "## Summary\n\nthe recap now posts the night.\n",
        ),
        merged(212, "feat(pns): another title", "no summary heading at all"),
    ];
    let lines = under(&rendered(&merges(&sources, None)), "NEW BEHAVIOR");
    assert_eq!(
        lines,
        [
            "NEW BEHAVIOR",
            "- #213 the recap now posts the night.",
            "- #212 feat(pns): another title",
        ],
        "the merges are not the section: {lines:?}"
    );
}

#[test]
fn a_line_citing_no_merge_pns_fetched_is_dropped_and_counted_rather_than_posted() {
    // RECEIPTS OR CUT, MECHANICALLY. pns holds the numbers it fetched, so a
    // line citing one it did not, or citing nothing at all, cannot be tied
    // to anything an operator can go and read. Both are dropped, and the
    // count of what is missing is the section's own, so cutting a line
    // never quietly shrinks the night's news.
    let sources = [
        merged(213, "a title", "## Summary\n\nthe first.\n"),
        merged(212, "a title", "## Summary\n\nthe second.\n"),
        merged(211, "a title", "## Summary\n\nthe third.\n"),
    ];
    let answered = [
        "#213 this one really merged".to_string(),
        "#999 this one never existed".to_string(),
        "and this one cites nothing at all".to_string(),
        "#2130 nor does a number that only looks like one".to_string(),
    ];
    let lines = under(
        &rendered(&merges(&sources, Some(&answered))),
        "NEW BEHAVIOR",
    );
    assert_eq!(
        lines,
        [
            "NEW BEHAVIOR",
            "- #213 this one really merged",
            "...and 2 more",
        ],
        "an uncited line was posted, or the count lied: {lines:?}"
    );
}

#[test]
fn a_receipt_with_anything_glued_to_either_end_names_a_different_source() {
    // A RECEIPT IS A WHOLE TOKEN, at both ends. A file name takes an extra
    // extension or a prefix and stays a plausible-looking name, so the
    // check that only guarded the trailing end let `checklist-s17.md.bak`
    // claim `checklist-s17.md`, which is the exact fabrication the comment
    // above it said was caught.
    let sources = [
        noted("checklist-s17.md", "# the real note\n"),
        noted("x.md", "# the other note\n"),
    ];
    let answered = [
        "checklist-s17.md.bak a note nobody wrote".to_string(),
        "old-x.md nor did anybody write this one".to_string(),
        "x.md this one was really read".to_string(),
    ];
    let lines = under(
        &rendered(&Externals {
            notes: External {
                found: Found::Read(&sources),
                answered: Some(&answered),
                truncated: false,
            },
            ..Externals::default()
        }),
        "CAUGHT BY REVIEW",
    );
    assert_eq!(
        lines,
        [
            "CAUGHT BY REVIEW, AND IMPLEMENTED",
            "- x.md this one was really read",
            "...and 1 more",
        ],
        "a name with something glued to it passed as a receipt: {lines:?}"
    );
}

#[test]
fn a_line_the_width_would_have_cut_into_a_receipt_is_judged_as_it_came() {
    // THE WIDTH MAY NOT DECIDE WHAT IS TRUE. `clipped` cuts to the section's
    // width and marks the cut, so a line positioning `#2130` at the edge
    // arrives at the check as `#213` and an ellipsis, and an out-of-set
    // receipt reads as one pns holds. Filtering the answer AS IT CAME is
    // what closes it; the boundary check alone does not, because the
    // ellipsis is a boundary.
    let sources = [
        merged(213, "a title", "## Summary\n\nthe first.\n"),
        merged(212, "a title", "## Summary\n\nthe second.\n"),
    ];
    let forged = format!("{} #2130 and whatever it says", "x".repeat(80));
    assert_eq!(
        crate::render::clipped(&forged, EXTERNAL_TEXT_CHARS)
            .chars()
            .rev()
            .take(5)
            .collect::<String>(),
        "…312#",
        "the fixture no longer clips into the number: {forged}"
    );
    let answered = ["#212 this one really merged".to_string(), forged];
    let lines = under(
        &rendered(&merges(&sources, Some(&answered))),
        "NEW BEHAVIOR",
    );
    assert_eq!(
        lines,
        [
            "NEW BEHAVIOR",
            "- #212 this one really merged",
            "...and 1 more",
        ],
        "a receipt the clip invented was believed: {lines:?}"
    );
}

#[test]
fn one_merge_vouches_for_one_line_and_the_rest_are_counted_as_missing() {
    // ONE SOURCE, ONE LINE. Membership alone let a model compress ten
    // merges under one number and have all four lines stand, so nine of
    // ten went unmentioned under a message that said six. A receipt is
    // spent by the first line that names it, and every source no surviving
    // line names is in the count.
    let sources: Vec<Sourced> = (0..10)
        .map(|which| merged(200 + which, "a title", "## Summary\n\nsomething shipped.\n"))
        .collect();
    let answered: Vec<String> = (0..4)
        .map(|_| "#200 the same one four times".to_string())
        .collect();
    let lines = under(
        &rendered(&merges(&sources, Some(&answered))),
        "NEW BEHAVIOR",
    );
    assert_eq!(
        lines,
        [
            "NEW BEHAVIOR",
            "- #200 the same one four times",
            "...and 9 more",
        ],
        "one receipt stood for four merges: {lines:?}"
    );
}

#[test]
fn the_remainder_counts_the_sources_no_surviving_line_names() {
    // COUNTED BY SOURCE, NEVER BY SUBTRACTING LINES. One line naming two
    // merges leaves one of three unmentioned, and subtracting the line
    // count would say two. The count is about the things pns read, which
    // is what the whole section is about.
    let sources = [
        merged(213, "a title", "## Summary\n\nthe first.\n"),
        merged(212, "a title", "## Summary\n\nthe second.\n"),
        merged(211, "a title", "## Summary\n\nthe third.\n"),
    ];
    let answered = ["#213 and #212 landed together".to_string()];
    let lines = under(
        &rendered(&merges(&sources, Some(&answered))),
        "NEW BEHAVIOR",
    );
    assert_eq!(
        lines,
        [
            "NEW BEHAVIOR",
            "- #213 and #212 landed together",
            "...and 1 more",
        ],
        "the remainder counted lines rather than sources: {lines:?}"
    );
}
