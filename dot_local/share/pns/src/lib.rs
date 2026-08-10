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
pub fn parse_count(raw: &str) -> Option<u64> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
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
}
