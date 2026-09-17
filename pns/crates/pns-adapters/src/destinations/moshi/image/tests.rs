use super::*;

/// The ink of one drawn line, as coordinates, so a test can assert what was
/// drawn without comparing an image byte for byte.
fn ink(line: &str, width: usize) -> Vec<(usize, usize)> {
    let mut lit = Vec::new();
    for row in 0..CELL_HEIGHT {
        let packed = pixel_row(line, row, width);
        for x in 0..width {
            if packed[x / 8] & (0x80 >> (x % 8)) == 0 {
                lit.push((x, row));
            }
        }
    }
    lit
}

#[test]
fn nothing_worth_drawing_renders_no_image_at_all() {
    // The caller falls back to the text card on `None`, so this is the
    // difference between a plain card and a card the endpoint refuses.
    for empty in ["", "\n", "\n\n", "   \n  "] {
        assert!(card_png(empty).is_none(), "`{empty:?}` drew an image");
    }
}

#[test]
fn a_drawn_line_carries_ink_and_a_blank_one_carries_none() {
    let width = COLUMNS * CELL_WIDTH * SCALE;
    assert!(!ink("pns", width).is_empty());
    assert!(ink("   ", width).is_empty(), "spaces drew ink");
}

#[test]
fn every_glyph_stays_inside_its_own_cell_and_the_rows_below_it_stay_clear() {
    // THE MUTANT THIS PINS: a cell width off by one, which overlaps the
    // neighbouring character and turns a recap line into a smear.
    let width = COLUMNS * CELL_WIDTH * SCALE;
    let cell = CELL_WIDTH * SCALE;
    for (x, row) in ink("MM", width) {
        assert!(x < 2 * cell, "ink at {x} is past the two cells drawn");
        assert!(row < font::HEIGHT, "ink on row {row}, which is the gap");
    }
    // The second M is the first one shifted by exactly one cell.
    let single = ink("M", width);
    let mut pair = ink("MM", width);
    let mut both = [
        single.clone(),
        single.iter().map(|(x, row)| (x + cell, *row)).collect(),
    ]
    .concat();
    pair.sort_unstable();
    both.sort_unstable();
    assert_eq!(pair, both);
}

#[test]
fn a_line_past_the_column_cap_wraps_at_a_space_rather_than_mid_word() {
    let long = "alpha ".repeat(20);
    let lines = wrapped(&long);
    assert!(lines.len() > 1, "a 120 character line stayed on one row");
    for line in &lines {
        assert!(line.chars().count() <= COLUMNS, "`{line}` is too wide");
        assert!(!line.starts_with(' ') && !line.ends_with(' '), "`{line}`");
    }
    assert_eq!(
        lines.join(" ").split_whitespace().count(),
        20,
        "wrapping lost or duplicated a word"
    );
}

#[test]
fn a_single_word_longer_than_the_line_is_broken_rather_than_left_too_wide() {
    let word = "x".repeat(COLUMNS + 5);
    let lines = wrapped(&word);
    assert_eq!(lines, ["x".repeat(COLUMNS), "xxxxx".to_string()]);
}

#[test]
fn the_message_keeps_its_own_line_breaks() {
    assert_eq!(wrapped("one\ntwo\n\nthree"), ["one", "two", "", "three"]);
}

#[test]
fn a_message_taller_than_the_image_says_how_many_lines_it_left() {
    // THE MUTANT THIS PINS: a silent `truncate`, on the one renderer whose
    // entire purpose is that the card stopped showing everything.
    let tall = "line\n".repeat(MAX_ROWS + 10);
    let lines = wrapped(&tall);
    assert_eq!(lines.len(), MAX_ROWS);
    assert!(
        lines.last().unwrap().starts_with("... 11 more lines"),
        "{:?}",
        lines.last()
    );
}

#[test]
fn the_image_is_a_png_of_the_size_the_grid_says() {
    let png = card_png("pns · missed\n2 events, 1 missed").expect("two lines draw");
    assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap()) as usize;
    assert_eq!(
        width,
        "2 events, 1 missed".chars().count() * CELL_WIDTH * SCALE
    );
    assert_eq!(height, 2 * CELL_HEIGHT * SCALE);
}

#[test]
fn a_whole_recap_card_renders_well_inside_the_upload_ceiling() {
    // TEN MEGABYTES is the documented per-upload ceiling. The widest, tallest
    // image this renderer can produce has to be nowhere near it, or the
    // ceiling becomes a delivery failure nobody predicted.
    let widest = format!("{}\n", "M".repeat(COLUMNS)).repeat(MAX_ROWS + 5);
    let png = card_png(&widest).expect("the largest image draws");
    assert!(
        png.len() < 1_000_000,
        "the largest image is {} bytes",
        png.len()
    );
}
