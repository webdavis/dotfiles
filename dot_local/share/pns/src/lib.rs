//! pns: the decision core plus its thin edges.
//!
//! THE SPLIT THAT MATTERS. The decision modules (`surface`, `presence`,
//! `routing`, `render`, `pulse`, `safety`) are total functions of their
//! arguments: no
//! network, no files, no clock, no environment. That is what makes them
//! testable one behavior at a time, in microseconds, without stubbing a
//! subprocess. The edges (`system` reads the machine, `config` reads the
//! file) keep their IO one seam away from a pure parser, and the engine
//! binary will own the wiring.
//!
//! The decision modules never print, exit, or read the environment. A caller
//! decides what to do with a verdict, and the composition root is where
//! wiring lives.

pub mod args;
pub mod channels;
pub mod config;
pub mod doctor;
pub mod engine;
pub mod home;
pub mod hooks;
pub mod presence;
pub mod probes;
pub mod pulse;
pub mod quiet;
pub mod registry;
pub mod render;
pub mod routing;
pub mod safety;
pub mod surface;
pub mod system;

/// A plain decimal count, or `None` when the text is not one.
///
/// The single gate every numeric reading passes: an idle clock, a signal's
/// age, an elapsed time. It is deliberately STRICTER than the standard integer
/// parse, which accepts a leading `+` and surrounding shapes this must not:
/// a reading that is not plain digits is UNKNOWN, and each caller states its
/// own fail direction for unknown rather than inheriting a coerced number.
///
/// A LEADING ZERO IS UNKNOWN RATHER THAN EITHER BASE, and that is a deliberate
/// divergence from the shell this ports. The shell validates the same
/// digits-only shape and then reads the digits inside `(( ))`, where `0600` is
/// OCTAL 384; this parse would say 600. Measured, that disagreement costs an
/// event: for an idle of 500 against a desk threshold of `0600` the shell
/// sends the phone card and a decimal reading DROPS it. Octal parity was not
/// the answer chosen, because it cements a base nobody writing a threshold
/// intended. Refusing the numeral is: the idle clock reads unknown as away and
/// PUSHES, so the ambiguity costs a duplicate card rather than a lost one. The
/// only other reading taken through here, a session's inbound byte count, drops
/// the row instead, which declines to vouch for a phone rather than inventing a
/// signal. A bare `0` carries no ambiguity and stays a count.
///
/// A COUNT PAST WHAT THE SHELL CAN HOLD IS UNKNOWN for the same reason, and it
/// is why a u64 function carries an i64-shaped ceiling. The bound mirrors the
/// SHELL'S arithmetic, not this crate's type: `(( ))` is signed 64-bit, so
/// measured on this host it reads 9223372036854775808 as -9223372036854775808
/// and 18446744073709551615 as -1. A negative threshold is below every idle
/// reading, so the shell SENDS those cards while parsing the numeral as itself
/// suppresses them. Refusing above `i64::MAX` puts both back on the push.
///
/// The ceiling is exactly one step past the largest value the shell counts up
/// to, so nothing it can hold is shaved off. Above `u64::MAX` needs no rule of
/// its own, because the parse already fails there and pushes. The two can still
/// disagree in that band: those numerals wrap modulo 2^64 to anything at all,
/// and a wrap landing on a large positive has the shell SUPPRESS where this
/// pushes (measured: 2^64 reads as 0 and sends, 2^64 + 10^12 reads as a
/// trillion and suppresses). That residue is left deliberately, because it runs
/// the OTHER way: it costs a duplicate card, never a dropped one.
pub fn parse_count(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return None;
    }
    let count: u64 = raw.parse().ok()?;
    (count <= SHELL_ARITHMETIC_MAX).then_some(count)
}

/// The largest count the shell this ports can hold, which is `i64::MAX` in the
/// u64 the parse returns. Above this its arithmetic wraps into the negatives.
const SHELL_ARITHMETIC_MAX: u64 = i64::MAX as u64;

#[cfg(test)]
mod tests {
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
}
