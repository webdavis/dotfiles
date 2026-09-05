//! Reading a plain decimal count out of text some other program wrote.
//!
//! The one numeric gate every probe reading, age and elapsed time passes, and
//! the reason it is policy rather than a parse helper: what it REFUSES is a
//! decision about which way an unreadable reading fails.

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
mod tests;
