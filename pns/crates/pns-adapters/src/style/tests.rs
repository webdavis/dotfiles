//! The house style: when it paints, and what shape it draws.

use super::*;

/// Does this line carry an escape sequence?
fn painted(line: &str) -> bool {
    line.contains('\u{1b}')
}

#[test]
fn a_terminal_with_nothing_asking_otherwise_is_painted() {
    assert_eq!(Paint::decide(false, true), Paint::Color);
}

#[test]
fn a_destination_that_is_not_a_terminal_is_never_painted() {
    // A pipe or a file takes the escape sequence as content, so this is the
    // signal that matters most: it is the difference between decoration and
    // corruption.
    assert_eq!(Paint::decide(false, false), Paint::Plain);
}

#[test]
fn the_flag_turns_it_off_on_a_terminal() {
    assert_eq!(Paint::decide(true, true), Paint::Plain);
}

#[test]
fn plain_mode_keeps_the_shape_and_drops_only_the_paint() {
    let plain = heading(Paint::Plain, "Channels", "one test send");
    let color = heading(Paint::Color, "Channels", "one test send");
    assert!(!painted(&plain));
    assert!(painted(&color));
    assert!(plain.contains("◆ Channels"), "{plain}");
    assert!(plain.contains("one test send"), "{plain}");
    // The painted line says the same words; only escape sequences separate it
    // from the plain one.
    let stripped: String = color
        .split('\u{1b}')
        .map(|part| part.split_once('m').map_or(part, |(_, rest)| rest))
        .collect();
    assert_eq!(stripped, plain);
}

#[test]
fn a_row_indents_by_the_amount_it_is_given_and_keeps_its_glyph_plain() {
    let line = row(Paint::Plain, Tone::Bad, "✗", 2, "mobile: FAILED");
    assert_eq!(line, "  ✗ mobile: FAILED");
    let deeper = row(Paint::Plain, Tone::Quiet, "→", 4, "detail");
    assert_eq!(deeper, "    → detail");
}

#[test]
fn a_rule_is_the_frame_s_own_width() {
    assert_eq!(rule(Paint::Plain).chars().count(), width());
}

#[test]
fn a_terminal_roomier_than_the_ceiling_still_gets_the_ceiling() {
    // What the old fixed constant was for, and the half of it worth keeping:
    // two roomy panes side by side render the identical report.
    assert_eq!(clamped(Some(200)), WIDEST);
    assert_eq!(clamped(Some(WIDEST)), WIDEST);
}

#[test]
fn a_terminal_narrower_than_the_ceiling_gets_its_own_width() {
    // THE BUG THIS FIXES. At a fixed sixty, a fifty-column pane wrapped every
    // border line and the box became line noise.
    assert_eq!(clamped(Some(50)), 50);
    assert_eq!(clamped(Some(NARROWEST)), NARROWEST);
}

#[test]
fn a_terminal_narrower_than_the_floor_is_drawn_at_the_floor() {
    // Cramped, with its content truncated by `pad`, rather than a broken box.
    assert_eq!(clamped(Some(1)), NARROWEST);
    assert_eq!(clamped(Some(NARROWEST - 1)), NARROWEST);
}

#[test]
fn a_destination_with_no_width_to_report_is_drawn_at_the_ceiling() {
    // A pipe, a file, a launchd job. Redirected output must not depend on the
    // size of whatever window launched the process.
    assert_eq!(clamped(None), WIDEST);
}

#[test]
fn a_header_opens_blank_then_names_its_invocation_its_labelled_lines_and_a_rule() {
    let lines = header(
        Paint::Plain,
        "pns doctor",
        &[HeaderLine {
            label: "Note",
            text: "every gate is bypassed",
        }],
    );
    // A LEADING BLANK, so the report does not begin flush against the prompt
    // the operator just typed.
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "pns doctor");
    assert_eq!(lines[2], "Note   every gate is bypassed");
    assert_eq!(lines[3].chars().count(), width());
    // The rule ends it. The gap below belongs to whatever section opens next.
    assert_eq!(lines.len(), 4);
}

#[test]
fn header_labels_line_up_in_one_column_however_wide_the_widest_is() {
    // Ragged labels read as unrelated lines. One column is what makes them
    // read as a table of facts about the same report.
    let lines = header(
        Paint::Plain,
        "pns tap --info",
        &[
            HeaderLine {
                label: "About",
                text: "the marker your phone touches",
            },
            HeaderLine {
                label: "Verified on",
                text: "iOS 26.2",
            },
        ],
    );
    assert_eq!(lines[2], "About         the marker your phone touches");
    assert_eq!(lines[3], "Verified on   iOS 26.2");
}

#[test]
fn a_header_carries_the_whole_invocation_so_the_reader_knows_which_flag_ran() {
    // `pns tap` and `pns tap --info` print different reports. A header naming
    // only the subcommand leaves the reader to work out which one they got.
    let lines = header(Paint::Plain, "pns tap --info", &[]);
    assert_eq!(lines[1], "pns tap --info");
}

#[test]
fn a_header_draws_no_box_at_any_width() {
    // A BOX NEEDS FOUR SIDES TO LINE UP, so any terminal narrower than its
    // content mangles it. A rule has one side and cannot be mangled.
    let lines = header(
        Paint::Plain,
        "pns tap --install",
        &[HeaderLine {
            label: "Steps",
            text: "1. This Mac    the authorized_keys line",
        }],
    );
    for glyph in ['\u{256d}', '\u{256e}', '\u{2570}', '\u{256f}', '\u{2502}'] {
        assert!(
            !lines.iter().any(|line| line.contains(glyph)),
            "{glyph:?} in {lines:?}"
        );
    }
}

#[test]
fn a_header_line_is_never_truncated_so_a_narrow_terminal_wraps_it_instead() {
    // TRUNCATION LOSES THE FACT. Wrapping costs a line and keeps it, and the
    // reader can always widen the window.
    let long = "x".repeat(200);
    let lines = header(
        Paint::Plain,
        "pns doctor",
        &[HeaderLine {
            label: "Note",
            text: &long,
        }],
    );
    assert_eq!(lines[2], format!("Note   {long}"));
}
