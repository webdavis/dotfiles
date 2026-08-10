//! pns decision core: the total functions the engine wraps with IO.
//!
//! WHY THIS IS A LIBRARY OF ITS OWN. Everything here is a function of its
//! arguments: no network, no files, no clock, no environment. That is what
//! makes it testable one behavior at a time, in microseconds, without stubbing
//! a subprocess. The binary keeps the impure half (reading the idle probe,
//! spawning channels) and this keeps the decisions.
//!
//! Nothing here prints diagnostics or exits, and nothing here reads a variable
//! out of the environment. A caller decides what to do with a verdict, and the
//! composition root is where wiring lives.

pub mod presence;
pub mod probes;
pub mod pulse;
pub mod render;
pub mod routing;
pub mod safety;

/// A plain decimal count, or `None` when the text is not one.
///
/// The single gate every numeric reading passes: an idle clock, a byte floor,
/// an elapsed time. It is deliberately STRICTER than the standard integer
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
/// intended. Refusing the numeral is: unknown fails OPEN into a push at every
/// site that consumes this, so the ambiguous reading costs a duplicate card
/// instead of a lost one. A bare `0` carries no ambiguity and stays a count.
///
/// KNOWN LIMIT, still open: a numeral above `i64::MAX` but within `u64` reads
/// here as itself while the shell's arithmetic wraps it negative, which lands
/// in that same push-suppressing direction. Nothing above `u64::MAX` parses,
/// so the gap is exactly that band.
pub fn parse_count(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return None;
    }
    raw.parse().ok()
}

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
    fn an_octal_looking_threshold_still_pushes_because_it_reads_as_unknown() {
        // The whole point of the unknown arm, measured end to end: the shell
        // sends the phone card for an idle of 500 against a desk threshold of
        // `0600`, because 500 is not below octal 384. Reading it as decimal
        // 600 would drop that push instead.
        assert!(crate::routing::wants_phone(
            Some(500),
            parse_count("0600"),
            false,
            false,
            false,
        ));
    }
}
