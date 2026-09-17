//! The one font the card image is drawn with: 5 by 7 pixels per glyph, for
//! the printable run of ASCII, as data in this file.
//!
//! EMBEDDED RATHER THAN LOADED. pns is installed with `cargo install` on
//! machines whose fonts it knows nothing about, and a card image that
//! depends on a font file is a card that renders differently, or not at all,
//! on the next machine. Seven bytes a glyph is the whole cost.
//!
//! FIVE BITS WIDE, LEFT COLUMN IN BIT 4, so a row reads left to right the
//! way the literal below is written.

/// The glyph's width and height in pixels.
pub(super) const WIDTH: usize = 5;
pub(super) const HEIGHT: usize = 7;

/// The rows of one character, or the fallback for one this font has no glyph
/// for.
///
/// TWO FALLBACKS AND NOT ONE. `·` is in every card title pns composes
/// (`render::title` joins with it), so it gets the glyph it deserves rather
/// than becoming a question mark three times per line; everything else
/// outside printable ASCII becomes `?`, which says a character was there and
/// could not be drawn instead of silently dropping it.
pub(super) fn glyph(character: char) -> [u8; HEIGHT] {
    match character {
        '\u{20}'..='\u{7e}' => GLYPHS[character as usize - 0x20],
        '\u{b7}' => MIDDLE_DOT,
        _ => GLYPHS['?' as usize - 0x20],
    }
}

/// The interpunct, which no ASCII table has and every pns title carries.
const MIDDLE_DOT: [u8; HEIGHT] = [
    0b00000, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000, 0b00000,
];

/// Every printable ASCII glyph, in code-point order from the space.
///
/// ONE ROW PER GLYPH, and `rustfmt` is told to leave it that way: the binary
/// literals ARE the picture, and a formatter that wraps them three to a line
/// turns a readable bitmap into 285 lines nobody can check by eye.
#[rustfmt::skip]
const GLYPHS: [[u8; HEIGHT]; 95] = [
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
    [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
    [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100],
    [0b11001, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b10011],
    [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
    [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
    [0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000],
    [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
    [0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000],
    [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
    [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
    [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
    [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b00100, 0b01000],
    [0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001],
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
    [0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000],
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100],
    [0b01110, 0b10001, 0b00001, 0b01111, 0b10101, 0b10101, 0b01111],
    [0b00100, 0b01010, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
    [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
    [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10011, 0b01111],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
    [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
    [0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001],
    [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
    [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000],
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
    [0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
    [0b00000, 0b00000, 0b01111, 0b10000, 0b10000, 0b10000, 0b01111],
    [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111],
    [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
    [0b00110, 0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000],
    [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b00010, 0b00000, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
    [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
    [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001],
    [0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000],
    [0b00000, 0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001],
    [0b00000, 0b00000, 0b10110, 0b11000, 0b10000, 0b10000, 0b10000],
    [0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
    [0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01001, 0b00110],
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b01111],
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
    [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
    [0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
    [0b00110, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00110],
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    [0b01100, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01100],
    [0b00000, 0b01001, 0b10101, 0b10010, 0b00000, 0b00000, 0b00000],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_printable_ascii_character_has_ink_except_the_space() {
        // THE MUTANT THIS PINS: a table row typed as all zeroes, which draws
        // a blank where a character was and is invisible in every other
        // test, since a card image is never compared byte for byte.
        for code in 0x21u8..=0x7e {
            let character = code as char;
            assert!(
                glyph(character).iter().any(|row| *row != 0),
                "`{character}` draws nothing"
            );
        }
        assert_eq!(glyph(' '), [0; HEIGHT], "the space draws ink");
    }

    #[test]
    fn no_glyph_reaches_past_the_five_columns_it_is_given() {
        // A sixth bit would bleed into the next character's cell.
        for code in 0x20u8..=0x7e {
            for row in glyph(code as char) {
                assert_eq!(row >> WIDTH, 0, "`{}` is wider than its cell", code as char);
            }
        }
    }

    #[test]
    fn the_interpunct_is_drawn_and_everything_else_unknown_becomes_a_question_mark() {
        assert_eq!(glyph('·'), MIDDLE_DOT);
        assert_ne!(glyph('·'), glyph('?'));
        for unknown in ['あ', '→', '\u{0}', '\u{7f}', '\t'] {
            assert_eq!(
                glyph(unknown),
                glyph('?'),
                "`{unknown:?}` drew something else"
            );
        }
    }
}
