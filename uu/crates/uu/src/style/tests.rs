use super::*;

#[test]
fn plain_paint_emits_no_escape_sequence_anywhere() {
    // The whole point of the flag: output that reaches a file or a pipe must
    // carry text and nothing else.
    let plain = Paint::Plain;
    let printed = [
        plain.accent("uu doctor"),
        plain.faint("config"),
        section(plain, "Lanes", "what the weekly run updates"),
        row(plain, Tone::Good, "brew: on"),
        detail(plain, "program /usr/bin/brew"),
        rule(plain),
    ]
    .join("\n");
    assert!(!printed.contains('\u{1b}'), "{printed:?}");
}

#[test]
fn a_section_is_flush_left_and_its_rows_are_indented() {
    // The layout the operator asked for: headings found by scanning one column,
    // their contents set in from it.
    let plain = Paint::Plain;
    assert_eq!(
        section(plain, "Lanes", "what the weekly run updates"),
        "◆ Lanes ── what the weekly run updates"
    );
    assert_eq!(row(plain, Tone::Good, "brew: on"), "  · brew: on");
    assert_eq!(
        detail(plain, "program /usr/bin/brew"),
        "    program /usr/bin/brew"
    );
}

#[test]
fn a_section_with_no_blurb_stops_after_its_name() {
    // A trailing rule with nothing after it reads as text that failed to print.
    assert_eq!(section(Paint::Plain, "Schedule", ""), "◆ Schedule");
}

#[test]
fn the_tool_name_and_a_section_take_different_colors() {
    // Pink for the tool, teal for a section. One column, two levels, so the
    // colour is the only thing separating them and they must not match.
    let color = Paint::Color;
    let tool = color.accent("uu doctor");
    let group = section(color, "Lanes", "");
    assert!(tool.contains(ACCENT), "{tool:?}");
    assert!(group.contains(SECTION), "{group:?}");
    assert!(!group.contains(ACCENT), "{group:?}");
}

#[test]
fn a_header_labels_every_line_and_pads_the_labels_to_one_column() {
    let lines = header(
        Paint::Plain,
        "uu doctor",
        &[
            HeaderLine {
                label: "config",
                text: "~/.config/uu/config.toml",
            },
            HeaderLine {
                label: "run",
                text: "weekly",
            },
        ],
    );
    assert_eq!(lines[0], "", "a report opens with a blank line");
    assert_eq!(lines[1], "uu doctor");
    assert_eq!(lines[2], "config   ~/.config/uu/config.toml");
    assert_eq!(lines[3], "run      weekly");
}

#[test]
fn a_header_with_no_lines_is_the_invocation_and_a_rule() {
    let lines = header(Paint::Plain, "uu doctor", &[]);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "uu doctor");
    assert!(lines[2].starts_with('─'));
}

#[test]
fn every_tone_paints_a_different_mark() {
    // A row's mark is the only thing carrying its state, so two states that
    // paint identically would be one state as far as a reader is concerned.
    let marks: Vec<String> = [Tone::Good, Tone::Bad, Tone::Warn, Tone::Quiet]
        .into_iter()
        .map(|tone| row(Paint::Color, tone, "x"))
        .collect();
    let mut unique = marks.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), marks.len(), "{marks:?}");
}

#[test]
fn an_unmeasurable_width_is_the_ceiling_rather_than_the_floor() {
    // A pipe has no width. Answering with the floor would draw every redirected
    // report at twenty columns.
    assert_eq!(clamped(None), WIDEST);
}

#[test]
fn a_measured_width_is_clamped_at_both_ends() {
    assert_eq!(clamped(Some(200)), WIDEST);
    assert_eq!(clamped(Some(5)), NARROWEST);
    assert_eq!(clamped(Some(40)), 40);
}

#[test]
fn the_flag_wins_over_a_terminal() {
    assert_eq!(Paint::decide(true, true), Paint::Plain);
}

#[test]
fn a_destination_that_is_not_a_terminal_is_plain_without_the_flag() {
    assert_eq!(Paint::decide(false, false), Paint::Plain);
}
