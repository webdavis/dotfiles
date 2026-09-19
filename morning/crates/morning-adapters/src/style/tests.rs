//! The house style: when it paints, and what shape it draws.

use super::*;
use morning_domain::Line;

/// Does this line carry an escape sequence?
fn painted(line: &str) -> bool {
    line.contains('\u{1b}')
}

#[test]
fn a_terminal_with_nothing_asking_otherwise_is_painted() {
    assert_eq!(
        Paint::decide_with_env(false, true, None, None),
        Paint::Color
    );
}

#[test]
fn a_destination_that_is_not_a_terminal_is_never_painted() {
    // A pipe or a file takes the escape sequence as content, so this is the
    // signal that matters most: it is the difference between decoration and
    // corruption.
    assert_eq!(
        Paint::decide_with_env(false, false, None, None),
        Paint::Plain
    );
}

#[test]
fn the_flag_turns_it_off_on_a_terminal() {
    assert_eq!(Paint::decide_with_env(true, true, None, None), Paint::Plain);
}

#[test]
fn environment_values_disable_color_only_when_their_convention_says_so() {
    use std::ffi::OsStr;

    for (no_color, house_plain, expected) in [
        (Some(""), Some("0"), Paint::Color),
        (Some("0"), None, Paint::Plain),
        (None, Some("1"), Paint::Plain),
        (None, Some("true"), Paint::Color),
    ] {
        assert_eq!(
            Paint::decide_with_env(
                false,
                true,
                no_color.map(OsStr::new),
                house_plain.map(OsStr::new),
            ),
            expected,
            "NO_COLOR={no_color:?}, REPORT_LIB_PLAIN={house_plain:?}",
        );
    }
}

#[test]
fn a_terminal_roomier_than_the_ceiling_still_gets_the_ceiling() {
    assert_eq!(clamped(Some(200)), WIDEST);
    assert_eq!(clamped(Some(WIDEST)), WIDEST);
}

#[test]
fn a_terminal_narrower_than_the_ceiling_gets_its_own_width() {
    assert_eq!(clamped(Some(50)), 50);
    assert_eq!(clamped(Some(NARROWEST)), NARROWEST);
}

#[test]
fn a_terminal_narrower_than_the_floor_is_drawn_at_the_floor() {
    assert_eq!(clamped(Some(1)), NARROWEST);
    assert_eq!(clamped(Some(NARROWEST - 1)), NARROWEST);
}

#[test]
fn a_destination_with_no_width_to_report_is_drawn_at_the_ceiling() {
    assert_eq!(clamped(None), WIDEST);
}

#[test]
fn a_plain_two_section_page_matches_byte_for_byte() {
    let lines = vec![
        Line::Header("morning".to_string()),
        Line::Heading("Last apply".to_string()),
        Line::Row("OK at 2026-09-17T10:18:49Z".to_string()),
        Line::Heading("Pull requests".to_string()),
        Line::Unavailable("gh exited 1".to_string()),
    ];
    let page = render_at(Paint::Plain, &lines, WIDEST);
    assert_eq!(
        page,
        format!(
            "\n\
             morning\n\
             {rule}\n\
             \n\
             ◆ Last apply ──\n\
             \u{20}\u{20}OK at 2026-09-17T10:18:49Z\n\
             \n\
             ◆ Pull requests ──\n\
             \u{20}\u{20}unavailable: gh exited 1\n",
            rule = "─".repeat(WIDEST)
        )
    );
}

#[test]
fn the_header_is_painted_in_the_accent_and_stays_plain_when_asked() {
    let lines = vec![Line::Header("morning".to_string())];
    let plain = render_at(Paint::Plain, &lines, WIDEST);
    let color = render_at(Paint::Color, &lines, WIDEST);
    assert!(!painted(&plain));
    assert!(painted(&color));
    assert!(plain.contains("morning"), "{plain}");
}

#[test]
fn a_heading_is_painted_in_its_own_color_distinct_from_the_header() {
    let lines = vec![Line::Heading("Today".to_string())];
    let color = render_at(Paint::Color, &lines, WIDEST);
    assert!(color.contains(HEADING_COLOR), "{color}");
    assert!(!color.contains(ACCENT), "{color}");
}

#[test]
fn a_row_is_clipped_with_an_ellipsis_at_the_measured_width() {
    let lines = vec![Line::Row("y".repeat(200))];
    let page = render_at(Paint::Plain, &lines, 10);
    let row = page.lines().next_back().unwrap();
    // Two-space indent plus the clipped text fits the width.
    assert_eq!(row.chars().count(), 10);
    assert!(row.ends_with('…'), "{row}");
}

#[test]
fn a_row_within_the_width_is_not_touched() {
    let lines = vec![Line::Row("short".to_string())];
    let page = render_at(Paint::Plain, &lines, WIDEST);
    assert_eq!(page.lines().next_back().unwrap(), "  short");
}

#[test]
fn more_carries_the_held_back_count_and_stays_faint() {
    let lines = vec![Line::More(27)];
    let plain = render_at(Paint::Plain, &lines, WIDEST);
    assert_eq!(plain, "  ... 27 more\n");
}

#[test]
fn nothing_reads_as_the_bare_word() {
    let lines = vec![Line::Nothing];
    let plain = render_at(Paint::Plain, &lines, WIDEST);
    assert_eq!(plain, "  nothing\n");
}

#[test]
fn unavailable_labels_only_the_word_and_leaves_the_reason_plain() {
    let lines = vec![Line::Unavailable("gh exited 1".to_string())];
    let color = render_at(Paint::Color, &lines, WIDEST);
    assert!(color.contains(&format!("{WARN}unavailable:{RESET}")), "{color}");
    assert!(color.ends_with("gh exited 1\n"), "{color}");
}
