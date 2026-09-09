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
fn a_frame_is_the_declared_width_on_every_line() {
    let lines = frame(
        Paint::Plain,
        &[("pns doctor".to_string(), true), ("sub".to_string(), false)],
    );
    for line in &lines {
        assert_eq!(
            line.chars().count(),
            WIDTH,
            "a frame line must be exactly {WIDTH} wide: {line:?}"
        );
    }
    assert!(lines.first().is_some_and(|line| line.starts_with('╭')));
    assert!(lines.last().is_some_and(|line| line.ends_with('╯')));
}

#[test]
fn a_frame_keeps_its_shape_when_a_title_is_longer_than_the_frame() {
    // Cut rather than wrapped: a frame whose right edge moves is worse than a
    // title the reader can still recognize from its first fifty characters.
    let long = "x".repeat(200);
    let lines = frame(Paint::Plain, &[(long, true)]);
    for line in &lines {
        assert_eq!(line.chars().count(), WIDTH, "{line:?}");
    }
}

#[test]
fn a_frame_holds_its_width_with_multi_byte_content() {
    // Counted in characters, not bytes, or a title with an accent in it pushes
    // the right edge left by one column per accent.
    let lines = frame(Paint::Plain, &[("é".repeat(20), true)]);
    for line in &lines {
        assert_eq!(line.chars().count(), WIDTH, "{line:?}");
    }
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
    assert_eq!(rule(Paint::Plain).chars().count(), WIDTH);
}
