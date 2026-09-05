//! The count parse's own tests: the shapes it takes and the shapes it
//! refuses, each one naming the disagreement with the shell that motivates it.

use super::parse_count;

#[test]
fn plain_digits_are_a_count() {
    assert_eq!(parse_count("0"), Some(0));
    assert_eq!(parse_count("120"), Some(120));
}

#[test]
fn an_empty_reading_is_unknown() {
    assert_eq!(parse_count(""), None);
}

#[test]
fn a_garbled_reading_is_unknown_rather_than_zero() {
    assert_eq!(parse_count("garbled"), None);
    assert_eq!(parse_count("12a"), None);
}

#[test]
fn a_signed_reading_is_unknown_because_the_shell_guard_refused_it_too() {
    // `+12` is where the standard parse and the shell's `^[0-9]+$` disagree:
    // accepting it would turn an unreadable probe into a confident number.
    assert_eq!(parse_count("+12"), None);
    assert_eq!(parse_count("-5"), None);
}

#[test]
fn a_padded_reading_is_unknown() {
    assert_eq!(parse_count(" 12"), None);
    assert_eq!(parse_count("12\n"), None);
}

#[test]
fn a_count_too_large_to_hold_is_unknown_rather_than_wrapped() {
    assert_eq!(parse_count("99999999999999999999999"), None);
}

#[test]
fn a_leading_zero_numeral_is_unknown_because_its_base_is_ambiguous() {
    // The shell validates this same digits-only shape and then reads the
    // digits inside `(( ))`, where `0600` is octal 384. Answering 600 here
    // is the disagreement that SUPPRESSES a push, so neither reading is
    // given and the ambiguity is reported as unknown.
    assert_eq!(parse_count("0600"), None);
    assert_eq!(parse_count("007"), None);
    // A bare zero carries no ambiguity and stays a count.
    assert_eq!(parse_count("0"), Some(0));
}

#[test]
fn a_count_past_what_the_shell_can_hold_is_unknown_rather_than_read_as_positive() {
    // The shell's arithmetic is signed 64-bit, measured on this host: it
    // reads 9223372036854775808 as -9223372036854775808 and sends the phone
    // card, where reading the numeral as itself suppresses one.
    assert_eq!(parse_count("9223372036854775808"), None);
    assert_eq!(parse_count("18446744073709551615"), None);
}

#[test]
fn the_ceiling_sits_exactly_where_the_shell_stops_counting_up() {
    // One below the wrap is still a count in both, so the refusal starts a
    // single step later rather than shaving off a value the shell can hold.
    assert_eq!(
        parse_count("9223372036854775807"),
        Some(9_223_372_036_854_775_807)
    );
    assert_eq!(parse_count("9223372036854775808"), None);
}
