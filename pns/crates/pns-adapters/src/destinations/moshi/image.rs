//! The card's image: the whole message, drawn, because the phone card's text
//! is cut at `render::PREVIEW_MAX_CHARS` and flattened to one line.
//!
//! WHAT THE IMAGE IS FOR, in one sentence: it shows what the preview cut.
//! That is the same thing on every card type, which is why there is one
//! renderer here rather than a layout per card.
//!
//! A FIXED GRID AND NOTHING MEASURED. Every glyph is the same size, so a line
//! is as wide as its character count and wrapping is arithmetic; there is no
//! font metric to read, no locale to consult and nothing to install.

mod font;
mod png;

/// How wide a drawn line is allowed to get before it wraps.
///
/// SIXTY, which is a terminal line rather than a phone's. The image is opened
/// full screen from the notification, and the recap lines this draws are
/// composed against an ASCII width already.
const COLUMNS: usize = 60;

/// How many lines the image draws before it says how many it left.
const MAX_ROWS: usize = 40;

/// How many image pixels one glyph pixel becomes. A 5 by 7 glyph is legible
/// but thin on a retina screen, and doubling costs four times almost nothing.
const SCALE: usize = 2;

/// One character's cell: the glyph plus the gap that separates it from its
/// neighbour on each axis.
const CELL_WIDTH: usize = font::WIDTH + 1;
const CELL_HEIGHT: usize = font::HEIGHT + 2;

/// The message as a PNG, or `None` when there is nothing to draw.
pub(super) fn card_png(text: &str) -> Option<Vec<u8>> {
    let lines = wrapped(text);
    let columns = lines.iter().map(|line| line.chars().count()).max()?;
    if columns == 0 {
        return None;
    }
    let width = columns * CELL_WIDTH * SCALE;
    let mut scanlines = Vec::with_capacity(lines.len() * CELL_HEIGHT * SCALE);
    for line in &lines {
        for row in 0..CELL_HEIGHT {
            let scanline = pixel_row(line, row, width);
            scanlines.extend(std::iter::repeat_n(scanline, SCALE));
        }
    }
    Some(png::greyscale_1bit(width, &scanlines))
}

/// One row of pixels across one text line, packed one bit per pixel with a
/// set bit for white.
fn pixel_row(line: &str, row: usize, width: usize) -> Vec<u8> {
    let mut packed = vec![0xff; width.div_ceil(8)];
    if row >= font::HEIGHT {
        return packed; // the gap under the glyphs
    }
    for (cell, character) in line.chars().enumerate() {
        let bits = font::glyph(character)[row];
        for column in 0..font::WIDTH {
            // BIT 4 IS THE LEFT COLUMN, which is how the font table reads.
            if bits & (1 << (font::WIDTH - 1 - column)) == 0 {
                continue;
            }
            for pixel in 0..SCALE {
                let x = (cell * CELL_WIDTH + column) * SCALE + pixel;
                packed[x / 8] &= !(0x80 >> (x % 8));
            }
        }
    }
    packed
}

/// The message as drawable lines: its own line breaks kept, anything longer
/// than `COLUMNS` wrapped, and a count of whatever did not fit.
///
/// THE OVERFLOW IS NAMED, NEVER DROPPED. An image whose whole purpose is to
/// show what the card's 260 characters cut must not cut silently in turn.
fn wrapped(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut rest = line.trim_end();
        while rest.chars().count() > COLUMNS {
            let (taken, left) = split_at_column(rest);
            lines.push(taken.to_string());
            rest = left;
        }
        lines.push(rest.to_string());
    }
    if lines.len() > MAX_ROWS {
        let left = lines.len() - (MAX_ROWS - 1);
        lines.truncate(MAX_ROWS - 1);
        lines.push(format!(
            "... {left} more lines, the full form is in the log"
        ));
    }
    lines
}

/// One line's first `COLUMNS` columns and what is left, broken at the last
/// space that fits and mid-word only when there is no space to break at.
fn split_at_column(line: &str) -> (&str, &str) {
    let end = line
        .char_indices()
        .nth(COLUMNS)
        .map_or(line.len(), |(index, _)| index);
    let split = line[..end].rfind(' ').unwrap_or(end);
    let left = if split == end { end } else { split + 1 };
    (line[..split].trim_end(), &line[left..])
}

#[cfg(test)]
#[path = "image/tests.rs"]
mod tests;
