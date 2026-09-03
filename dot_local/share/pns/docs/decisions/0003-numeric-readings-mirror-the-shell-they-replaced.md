# 0003: A numeric reading is refused rather than coerced, and the refusals mirror the shell pns replaced

Status: accepted. Implemented by `parse_count` in `src/lib.rs`, which every numeric reading passes
through: an idle clock, a signal's age, an elapsed time, a session's inbound byte count.

## The rule

`parse_count` is deliberately stricter than the standard integer parse. Anything that is not a plain
decimal numeral is UNKNOWN, and each caller states its own fail direction for unknown rather than
inheriting a coerced number.

## The three refusals, and the measurements behind them

**A leading `+` or `-` is unknown.** The standard parse accepts a leading `+`; the shell's own `^[0-9]+$`
guard did not. Accepting it would turn an unreadable probe into a confident number.

**A leading zero is unknown rather than either base.** This is a deliberate divergence from the shell
being ported. The shell validates the same digits-only shape and then reads the digits inside `(( ))`,
where `0600` is octal 384, while a decimal reading says 600. Measured, that disagreement costs an event:
for an idle of 500 against a desk threshold of `0600` the shell sends the phone card and a decimal
reading DROPS it. Octal parity was considered and refused, because it cements a base nobody writing a
threshold intended. Refusing the numeral was chosen instead: the idle clock reads unknown as away and
PUSHES, so the ambiguity costs a duplicate card rather than a lost one. A bare `0` carries no ambiguity
and stays a count.

**A count above `i64::MAX` is unknown rather than wrapped.** The bound mirrors the SHELL'S arithmetic,
not this crate's type. `(( ))` is signed 64 bit, so measured on this host it reads 9223372036854775808 as
-9223372036854775808 and 18446744073709551615 as -1. A negative threshold is below every idle reading, so
the shell SENDS those cards while parsing the numeral as itself suppresses them. Refusing above
`i64::MAX` puts both back on the push. The ceiling sits exactly one step past the largest value the shell
counts up to, so nothing it can hold is shaved off.

## The residue that was accepted

Above `u64::MAX` the parse already fails and pushes, so it needs no rule. The two can still disagree in
that band: those numerals wrap modulo 2^64 to anything at all, and a wrap landing on a large positive has
the shell SUPPRESS where this pushes. Measured, 2^64 reads as 0 and sends, and 2^64 plus 10^12 reads as a
trillion and suppresses. That residue was left deliberately, because it runs the safe way: it costs a
duplicate card, never a dropped one.

## Consequence for the refactor

This is domain policy and belongs in the domain crate. It depends on nothing but `std`. The property
worth keeping under test is the direction: an unreadable reading must never silently become a confident
one, and each caller states which way unknown falls.
