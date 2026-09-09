use super::*;

/// Comment text is cut once, and everything before it survives whole.
#[test]
fn the_record_is_what_sits_before_the_first_comment_character() {
    assert_eq!(record_text(b"10.0.0.5\tnas # the box"), b"10.0.0.5\tnas ");
    assert_eq!(record_text(b"# nothing but a comment"), b"");
    assert_eq!(record_text(b"10.0.0.5\tnas"), b"10.0.0.5\tnas");
    assert_eq!(record_text(b"a#b#c"), b"a");
}

/// A leading run of blanks is skipped, because the resolver skips it: this is a
/// working record and a check refusing it would refuse a file that resolves.
#[test]
fn leading_blanks_do_not_hide_a_record() {
    assert!(is_record_for_address(
        b"  \t127.0.0.1 localhost",
        b"127.0.0.1"
    ));
    assert!(claims_name(b"  \t127.0.0.1 localhost", b"localhost"));
}

/// The address is the FIRST FIELD. A line whose first field is something else
/// is not a record for that address, whatever it holds later.
#[test]
fn an_address_in_a_name_column_is_not_the_record_s_address() {
    assert!(!is_record_for_address(b"10.0.0.1\t127.0.0.1", b"127.0.0.1"));
}

/// The gate this exists for: a comment-only line satisfied a `grep` and left a
/// machine with no name to resolve localhost through.
#[test]
fn a_comment_only_line_is_not_a_record() {
    assert!(!is_record_for_address(b"127.0.0.1  # decoy", b"127.0.0.1"));
    assert!(!is_record_for_address(b"127.0.0.1", b"127.0.0.1"));
    assert!(!is_record_for_address(b"", b"127.0.0.1"));
}

/// FIELD EQUALITY, never a word boundary: to grep, `.` and `-` end a word, so a
/// word-boundary filter also matched these, which are different hosts.
#[test]
fn a_name_that_merely_contains_the_pinned_one_is_not_a_claim() {
    for line in [
        b"10.0.0.5\tpin.example.test.evil".as_slice(),
        b"10.0.0.5\tother-pin.example.test",
        b"10.0.0.5\tpin.example.tes",
        b"10.0.0.5\txpin.example.test",
    ] {
        assert!(
            !claims_name(line, b"pin.example.test"),
            "{:?} read as a claim",
            String::from_utf8_lossy(line)
        );
    }
    assert!(claims_name(
        b"10.0.0.5\tpin.example.test",
        b"pin.example.test"
    ));
}

/// The address column is not a name column, so a line cannot claim its own
/// address as a name.
#[test]
fn the_address_column_is_never_a_claimed_name() {
    assert!(!claims_name(b"10.0.0.5\tnas", b"10.0.0.5"));
}

/// A name written after a `#` is a real alias to the resolver and is not read
/// as one here. This UNDER-claims on purpose: the line survives a rebuild
/// rather than being dropped on a reading that might be wrong.
#[test]
fn a_name_inside_comment_text_is_not_claimed() {
    assert!(!claims_name(
        b"10.0.0.5\tnas # pin.example.test",
        b"pin.example.test"
    ));
}

/// The one place a byte is dropped, and only on the one line where the resolver
/// never read it.
#[test]
fn the_unread_carriage_return_is_dropped_from_a_final_unterminated_record() {
    assert_eq!(
        without_unread_carriage_return(b"127.0.0.1\tlocalhost\r"),
        b"127.0.0.1\tlocalhost"
    );
}

/// A final unterminated COMMENT is not chopped by the resolver, so its carriage
/// return was read; removing it would edit a line the rebuild copies through.
#[test]
fn a_final_comment_keeps_its_carriage_return() {
    assert_eq!(without_unread_carriage_return(b"# a note\r"), b"# a note\r");
}

/// One byte, not a run of them, and only when it is a carriage return.
#[test]
fn nothing_else_is_dropped() {
    assert_eq!(
        without_unread_carriage_return(b"10.0.0.5\tnas"),
        b"10.0.0.5\tnas"
    );
    assert_eq!(
        without_unread_carriage_return(b"10.0.0.5\tnas\r\r"),
        b"10.0.0.5\tnas\r"
    );
    assert_eq!(without_unread_carriage_return(b""), b"");
    assert_eq!(without_unread_carriage_return(b"\r"), b"");
}

/// A carriage return in the MIDDLE of a line is an ordinary name byte to the
/// resolver and to this, so a CRLF file's names carry it and never match a
/// pin's own name. That is the CRLF limitation, stated as a test.
#[test]
fn a_carriage_return_inside_a_line_is_an_ordinary_name_byte() {
    assert!(!claims_name(b"10.0.0.5\tnas\r", b"nas"));
    assert!(claims_name(b"10.0.0.5\tnas\r", b"nas\r"));
}

/// Bytes, not text: a hosts file holds whatever the filesystem holds, and a NUL
/// or an invalid UTF-8 sequence is copied through rather than refused.
#[test]
fn a_line_that_is_not_utf8_is_read_without_complaint() {
    assert!(claims_name(b"10.0.0.5\tnas\xff", b"nas\xff"));
    assert!(claims_name(b"10.0.0.5\tnas\0junk", b"nas\0junk"));
}

/// Either name is a claim, which is how a pin owns its short name as well as
/// its fully qualified one.
#[test]
fn a_line_claiming_either_name_is_claimed() {
    let names: [&[u8]; 2] = [b"pin.example.test", b"pin"];
    assert!(claims_any(b"10.0.0.5\tpin.example.test", &names));
    assert!(claims_any(b"10.0.0.5\tpin", &names));
    assert!(claims_any(b"10.0.0.5\tsomething\tpin", &names));
    assert!(!claims_any(b"10.0.0.5\tnas", &names));
}
