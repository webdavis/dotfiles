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
    assert_eq!(closing[1].chars().count(), crate::style::WIDTH);
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
    assert_eq!(closing[2], "  Found 2 issues to address:");
    assert_eq!(closing[3], "  1. mobile: FAILED, push refused");
    assert_eq!(closing[4], "  2. hue: FAILED, no bridge");
}

#[test]
fn one_issue_is_not_pluralised() {
    let mut report = plain();
    report.item(&Item::row(Mark::Bad, "mobile: FAILED"));
    assert_eq!(report.close()[2], "  Found 1 issue to address:");
}

#[test]
fn the_frame_names_the_command_and_its_subtitle() {
    let report = plain();
    let frame = report.open("every suppression gate is bypassed");
    let joined = frame.join("\n");
    assert!(joined.contains("pns doctor"), "{joined}");
    assert!(
        joined.contains("every suppression gate is bypassed"),
        "{joined}"
    );
    for line in &frame {
        assert_eq!(line.chars().count(), crate::style::WIDTH, "{line:?}");
    }
}
