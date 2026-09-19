use super::*;

fn rows_of(lines: &[Line]) -> Vec<&str> {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Row(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_page_opens_with_the_header_then_one_heading_per_section() {
    let lines = render(
        "morning",
        &[
            Section::lines("Last apply", vec!["OK at 2026-09-17T10:18:49Z".into()]),
            Section::unavailable("Pull requests", "gh exited 1"),
        ],
        8,
    );
    assert!(matches!(&lines[0], Line::Header(text) if text == "morning"));
    assert!(matches!(&lines[1], Line::Heading(title) if title == "Last apply"));
    assert!(matches!(&lines[2], Line::Row(text) if text == "OK at 2026-09-17T10:18:49Z"));
    assert!(matches!(&lines[3], Line::Heading(title) if title == "Pull requests"));
    assert!(matches!(&lines[4], Line::Unavailable(reason) if reason == "gh exited 1"));
    assert_eq!(lines.len(), 5);
}

#[test]
fn an_empty_section_says_nothing_rather_than_going_missing() {
    let lines = render("morning", &[Section::lines("Applies owed", Vec::new())], 8);
    assert!(matches!(lines.last(), Some(Line::Nothing)));
}

#[test]
fn a_page_with_no_sections_is_the_header_alone() {
    assert_eq!(render("morning", &[], 8).len(), 1);
}

#[test]
fn a_section_longer_than_its_row_cap_keeps_the_first_rows_and_counts_the_rest() {
    let items: Vec<String> = (1..=30).map(|n| format!("item {n}")).collect();
    let lines = render(
        "morning",
        &[Section::lines("Operator's own items", items)],
        3,
    );
    assert_eq!(rows_of(&lines), vec!["item 1", "item 2", "item 3"]);
    assert!(matches!(lines.last(), Some(Line::More(27))));
}

#[test]
fn a_section_that_fits_within_its_row_cap_carries_no_more_line() {
    let lines = render(
        "morning",
        &[Section::lines("Today", vec!["a".into(), "b".into()])],
        8,
    );
    assert!(!lines.iter().any(|line| matches!(line, Line::More(_))));
}

#[test]
fn a_row_cap_of_zero_still_keeps_one_row() {
    let lines = render(
        "morning",
        &[Section::lines("Today", vec!["a".into(), "b".into()])],
        0,
    );
    assert_eq!(rows_of(&lines), vec!["a"]);
}
