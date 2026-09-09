//! Reading one line of `/etc/hosts` the way the RESOLVER reads it.
//!
//! This is a codec for somebody else's format, and its rules were measured
//! against the implementation that actually answers a lookup rather than read
//! off `hosts(5)`. Every departure from the obvious reading is recorded at the
//! function that makes it, with what the measurement was.
//!
//! BYTES, NOT TEXT. A hosts file is whatever the filesystem holds, and the two
//! reads this makes, splitting on separators and comparing name fields, are
//! byte comparisons in the resolver too. Reading it as UTF-8 would put a refusal
//! path in a root rewrite for a file the resolver parses without complaint, and
//! would silently rewrite bytes it could not decode.

/// Where a comment starts. Everything from here to the end of the line is not
/// part of the record.
pub const COMMENT: u8 = b'#';

/// The byte a CRLF file leaves at the end of every line.
pub const CARRIAGE_RETURN: u8 = b'\r';

/// What separates the items of a record. `hosts(5)`: "Items are separated by
/// any number of blanks and/or tab characters."
///
/// The newline is in the set because the resolver's own splitter carries it,
/// not because a line can hold one: the caller has already split on newlines by
/// the time anything here runs.
const SEPARATORS: [u8; 3] = *b" \t\n";

/// A record needs an address and at least one name. A line with fewer items is
/// not a record, whatever else it is.
const MINIMUM_FIELDS: usize = 2;

/// The part of `line` that carries the record: everything before the first
/// comment character.
///
/// THE ONE PLACE COMMENT TEXT IS CUT, so every predicate below reads the same
/// text and the divergence that cut creates has one home. The resolver would
/// read a name written after a `#` as a real alias and this does not, which is
/// a known limitation rather than a bug: it makes this UNDER-claim, so such a
/// line survives a rebuild instead of being dropped.
pub fn record_text(line: &[u8]) -> &[u8] {
    match line.iter().position(|byte| *byte == COMMENT) {
        Some(comment) => &line[..comment],
        None => line,
    }
}

/// The line, minus the carriage return the resolver never read.
///
/// ONLY EVER CALLED ON A FILE'S FINAL LINE WHEN THAT FILE HAS NO TERMINATOR,
/// which is the only place such a byte exists. The resolver's `_fsi_get_line`
/// chops the last byte of every non-comment line whether or not there is a
/// newline to chop, so on that one line it reads one byte less than the file
/// holds. A rebuild gives that byte back by writing the terminator.
///
/// Giving back a byte of the RECORD is the repair that matters: `localhos`
/// becomes `localhost` again. Giving back a CARRIAGE RETURN is not. It is a
/// line-ending artifact the resolver had already discarded, and handing it back
/// turns a name the machine resolves into one it cannot. Leaving it in cost this
/// machine its localhost, which is why the rule is here and not a footnote.
///
/// The comment exemption mirrors the resolver's own (`if (s[0] != '#')`): a
/// final unterminated comment is not chopped there, so its carriage return was
/// read rather than unread, and removing it would edit a line a rebuild
/// promises to copy through.
pub fn without_unread_carriage_return(line: &[u8]) -> &[u8] {
    if line.first() == Some(&COMMENT) {
        return line;
    }
    match line.split_last() {
        Some((&CARRIAGE_RETURN, rest)) => rest,
        _ => line,
    }
}

/// The record's items, in order, with comment text already cut.
///
/// A leading run of blanks is skipped, for the same reason the resolver skips
/// it: `  127.0.0.1 localhost` is a working record, and a check refusing it
/// would refuse a file the machine resolves fine.
pub fn fields(line: &[u8]) -> Vec<&[u8]> {
    record_text(line)
        .split(|byte| SEPARATORS.contains(byte))
        .filter(|field| !field.is_empty())
        .collect()
}

/// Is this line a record mapping exactly `address` to at least one name?
///
/// FIRST FIELD, NOT FIRST CHARACTER, and both halves of that matter. The
/// minimum-field test is what refuses a comment-only `127.0.0.1  # decoy`,
/// which once satisfied a `grep` gate and left a machine with no name to
/// resolve localhost through. The first-FIELD half is what refuses
/// `10.0.0.1<tab>127.0.0.1`, where the address is a NAME.
pub fn is_record_for_address(line: &[u8], address: &[u8]) -> bool {
    let fields = fields(line);
    fields.len() >= MINIMUM_FIELDS && fields[0] == address
}

/// Does this line claim `name` as one of its host names?
///
/// Name fields are the items AFTER the address. FIELD EQUALITY, never a `grep`
/// word boundary: to grep, `.` and `-` end a word, so a word-boundary filter
/// also matched `pin.example.test.evil` and `other-pin.example.test`, which are
/// DIFFERENT hosts that merely contain the pinned name.
pub fn claims_name(line: &[u8], name: &[u8]) -> bool {
    let fields = fields(line);
    fields.len() >= MINIMUM_FIELDS && fields[1..].contains(&name)
}

/// Does this line claim any of these names?
pub fn claims_any(line: &[u8], names: &[&[u8]]) -> bool {
    names.iter().any(|name| claims_name(line, name))
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
