//! What the doctor's report looks like, and what it remembers.

use super::*;
use pns_domain::doctor::Item;

fn plain() -> Report {
    Report::new(Paint::Plain)
}

#[test]
fn a_section_opens_with_a_blank_line_so_the_gap_belongs_to_it() {
    let mut report = plain();
    let lines = report.item(&Item::section("Channels", "one test send"));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "");
    assert!(lines[1].contains("◆ Channels"), "{:?}", lines[1]);
    assert!(lines[1].contains("one test send"), "{:?}", lines[1]);
}

#[test]
fn each_mark_gets_its_own_glyph() {
    let mut report = plain();
    for (mark, expected) in [
        (Mark::Good, "  ✓ text"),
        (Mark::Bad, "  ✗ text"),
        (Mark::Warn, "  ⚠ text"),
        (Mark::Note, "  · text"),
        (Mark::Detail, "    → text"),
    ] {
        let lines = report.item(&Item::row(mark, "text"));
        assert_eq!(lines, vec![expected.to_string()], "{mark:?}");
    }
}

#[test]
fn a_detail_row_is_indented_under_the_row_it_continues() {
    let mut report = plain();
    let row = report.item(&Item::row(Mark::Note, "parent"));
    let detail = report.item(&Item::row(Mark::Detail, "child"));
    assert!(detail[0].starts_with("    "), "{:?}", detail[0]);
    assert!(row[0].starts_with("  ·"), "{:?}", row[0]);
}

#[test]
fn a_report_with_nothing_wrong_closes_by_saying_so() {
    let report = plain();
    let closing = report.close();
    assert_eq!(closing[0], "");
    assert_eq!(closing[1].chars().count(), crate::style::width());
    assert_eq!(closing[2], "  ✓ nothing to act on");
}

#[test]
fn only_a_bad_row_reaches_the_closing_list() {
    let mut report = plain();
    for mark in [Mark::Good, Mark::Warn, Mark::Note, Mark::Detail] {
        report.item(&Item::row(mark, format!("{mark:?} row")));
    }
    assert_eq!(report.close()[2], "  ✓ nothing to act on");
}

#[test]
fn the_closing_list_numbers_every_bad_row_in_the_order_it_appeared() {
    // IT REPEATS ROWS ALREADY PRINTED, which is the point: the failing row has
    // scrolled off by the time the report ends.
    let mut report = plain();
    report.item(&Item::row(Mark::Bad, "mobile: FAILED, push refused"));
    report.item(&Item::row(Mark::Good, "hermes: sent"));
    report.item(&Item::row(Mark::Bad, "hue: FAILED, no bridge"));
    let closing = report.close();
    assert_eq!(closing[2], "  2 issues to fix:");
    assert_eq!(closing[3], "  1. mobile: FAILED, push refused");
    assert_eq!(closing[4], "  2. hue: FAILED, no bridge");
}

#[test]
fn one_issue_is_not_pluralised() {
    let mut report = plain();
    report.item(&Item::row(Mark::Bad, "mobile: FAILED"));
    assert_eq!(report.close()[2], "  1 issue to fix:");
}

#[test]
fn the_header_names_the_command_and_labels_the_line_under_it() {
    // A BARE SENTENCE UNDER A COMMAND NAME reads like an error. The label is
    // what tells the reader it is a caveat about the report below it.
    let report = plain();
    let opening = report.open("every suppression gate is bypassed");
    assert_eq!(opening[0], "", "the report opens clear of the prompt");
    assert_eq!(opening[1], "pns doctor");
    assert_eq!(opening[2], "Note   every suppression gate is bypassed");
    assert_eq!(opening[3].chars().count(), crate::style::width());
}

#[test]
fn the_opening_draws_no_box() {
    // A box needs four sides to line up, so a narrow terminal mangles it.
    let opening = plain()
        .open("every suppression gate is bypassed")
        .join("\n");
    for glyph in ['\u{256d}', '\u{256e}', '\u{2570}', '\u{256f}', '\u{2502}'] {
        assert!(!opening.contains(glyph), "{glyph:?} in {opening}");
    }
}

#[test]
fn a_row_drops_the_command_name_the_sentence_carries_for_other_readers() {
    // The frame already said `pns doctor`, and the section heading says which
    // part of it this is, so repeating the command once per row is the noise
    // the sections exist to remove.
    let mut report = Report::new(Paint::Plain);
    let lines = report.item(&Item::Row {
        mark: Mark::Note,
        text: "pns doctor: the daemon is running, pid 4321, 2 jobs scheduled".to_string(),
    });
    assert_eq!(
        lines,
        vec!["  · the daemon is running, pid 4321, 2 jobs scheduled"]
    );
}

#[test]
fn a_row_that_never_carried_the_prefix_is_printed_exactly_as_written() {
    let mut report = Report::new(Paint::Plain);
    let lines = report.item(&Item::Row {
        mark: Mark::Good,
        text: "mobile: sent, pushed the card".to_string(),
    });
    assert_eq!(lines, vec!["  ✓ mobile: sent, pushed the card"]);
}

#[test]
fn the_closing_list_repeats_the_stripped_row_and_not_the_attributed_one() {
    // The list quotes rows already printed, so an entry still carrying the
    // command name would read as a different sentence from the row above it.
    let mut report = Report::new(Paint::Plain);
    report.item(&Item::Row {
        mark: Mark::Bad,
        text: "pns doctor: hermes: FAILED, post FAILED HTTP 000".to_string(),
    });
    let close = report.close();
    assert!(
        close
            .iter()
            .any(|line| line == "  1. hermes: FAILED, post FAILED HTTP 000"),
        "{close:?}"
    );
}
