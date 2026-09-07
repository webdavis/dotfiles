//! What may appear in a line somebody else wrote.

/// One line of somebody else's answer, made safe to put in the message: every
/// kind of whitespace becomes one space, every control byte and every INVISIBLE
/// character is dropped whole, and what is left is capped to a timeline line's
/// width.
///
/// THE FORMAT CHARACTERS GO TOO, and `char::is_control` does not reach them:
/// U+202E RIGHT-TO-LEFT OVERRIDE and U+200B ZERO WIDTH SPACE are Unicode
/// category Cf, which is neither control nor whitespace, and Discord honours
/// the override by rendering a line in an order nobody wrote it in. The ranges
/// below are that category's bidi, zero-width, invisible-operator and
/// byte-order marks; anything a reader cannot see has no business in a line
/// pns signs its name to.
///
/// DROPPED RATHER THAN ESCAPED, which is the opposite of the decision log's
/// rule and for the opposite reason: that one is read on a terminal by an
/// operator asking what happened, so an escape is evidence, while this is a
/// sentence posted to a chat channel, where `\u{1b}` in the middle of a line is
/// only noise.
///
/// AND THE HEAD IS WHAT SURVIVES THE WIDTH, which is why the cut is `clipped`
/// and not the flatten's own cap. `flatten_reply` keeps a TURN's tail, because
/// a turn states its conclusion at the end; this is a line somebody composed,
/// whose beginning names what it is about, and `fit` goes on to cut the same
/// line from the same end. Cutting the two ends in turn would leave the middle
/// of a sentence and nothing to say which part of it that was.
pub(super) fn safe_line(line: &str, max_chars: usize) -> String {
    let printable: String = line
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .filter(|character| !character.is_control() && !is_invisible(*character))
        .collect();
    crate::render::clipped(
        &crate::render::flatten_reply(&printable, usize::MAX),
        max_chars,
    )
}
/// Whether a character is one the reader cannot see: every Unicode FORMAT
/// (Cf) code point, stated as ranges because std has no category lookup and
/// this crate takes no dependency for one.
///
/// EVERY Cf CODE POINT, not the bidi and zero-width ones alone: U+061C ARABIC
/// LETTER MARK was found missing (neither whitespace nor `char::is_control`,
/// same as the ranges beside it) by a review that read this doc comment's own
/// claim literally and checked it against the category it names. A second
/// review then found the claim still overstated after that fix: two of the
/// ranges disagreed with the standard (U+0890..U+0891 absent, U+13430
/// range truncated at U+13438), nine code points short of the 170 the
/// category actually holds. The set below is the full category as of
/// Unicode 17.0, transcribed again from the standard's own
/// `DerivedGeneralCategory.txt`, and `is_invisible_agrees_with_unicode_17_0_across_every_code_point`
/// checks it against an independently transcribed copy of that same file for
/// every valid `char`, so a third gap fails a test rather than waiting on a
/// third review.
///
/// PUB FOR ONE OTHER READER, `main.rs`'s automatic model-switch card and its
/// `ConfigChange` sibling: a payload field that is not free text still
/// carries whatever bytes a harness sends, and a reorder character surviving
/// `flattened` (which only strips whitespace and `char::is_control`, the Cc
/// set, never Cf) would let a name or a path render backwards.
pub fn is_invisible(character: char) -> bool {
    matches!(
        character,
        '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}
