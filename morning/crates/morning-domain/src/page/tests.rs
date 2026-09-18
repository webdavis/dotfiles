use super::*;

#[test]
fn a_full_page_frames_the_heading_and_indents_every_row() {
    let page = render(
        "morning, 2026-09-17",
        &[
            Section::lines("Last apply", vec!["OK at 2026-09-17T10:18:49Z".into()]),
            Section::unavailable("Pull requests", "gh exited 1"),
        ],
        8,
    );
    assert_eq!(
        page,
        "╭──────────────────────────────────────────────────────────────────╮\n\
         │ morning, 2026-09-17                                              │\n\
         ╰──────────────────────────────────────────────────────────────────╯\n\
         \n\
         Last apply\n\
         \u{20}\u{20}OK at 2026-09-17T10:18:49Z\n\
         \n\
         Pull requests\n\
         \u{20}\u{20}unavailable: gh exited 1\n"
    );
}

#[test]
fn an_empty_section_says_nothing_rather_than_going_missing() {
    let page = render("morning", &[Section::lines("Applies owed", Vec::new())], 8);
    assert!(page.ends_with("Applies owed\n  nothing\n"), "{page}");
}

#[test]
fn a_page_with_no_sections_is_the_frame_alone() {
    assert_eq!(render("morning", &[], 8).lines().count(), 3);
}

#[test]
fn a_heading_wider_than_the_frame_is_cut_rather_than_wrapped() {
    let page = render(&"x".repeat(200), &[], 8);
    assert!(
        page.lines().all(|line| line.chars().count() == 68),
        "{page}"
    );
}

#[test]
fn a_section_longer_than_the_page_keeps_its_first_rows_and_counts_the_rest() {
    let items: Vec<String> = (1..=30).map(|n| format!("item {n}")).collect();
    let page = render(
        "morning",
        &[Section::lines("Operator's own items", items)],
        3,
    );
    assert!(page.contains("  item 3\n  ... 27 more\n"), "{page}");
    assert!(!page.contains("item 4"), "{page}");
}

#[test]
fn a_row_wider_than_the_page_is_cut_rather_than_left_to_wrap() {
    let page = render(
        "morning",
        &[Section::lines("Applies owed", vec!["y".repeat(200)])],
        8,
    );
    let row = page.lines().last().unwrap();
    assert_eq!(row.chars().count(), 68);
    assert!(row.ends_with("..."), "{row}");
}
