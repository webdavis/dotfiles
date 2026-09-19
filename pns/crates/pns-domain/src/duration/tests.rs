//! What a duration may be typed as, and what each refusal says.

use super::parse_duration;
use std::ops::RangeInclusive;
use std::time::Duration;

#[test]
fn a_count_and_its_unit_are_that_much_time() {
    // ALL FOUR UNITS, over a range wide enough to hold them, so the units
    // are what this pins rather than one field's bounds.
    let wide = Duration::from_millis(1)..=Duration::from_secs(86_400);
    for (typed, held) in [
        ("500ms", Duration::from_millis(500)),
        ("45s", Duration::from_secs(45)),
        ("30m", Duration::from_secs(1_800)),
        ("2h", Duration::from_secs(7_200)),
    ] {
        assert_eq!(
            parse_duration("quiet duration", typed, wide.clone()),
            Ok(held),
            "typed: {typed:?}"
        );
    }
}

#[test]
fn a_duration_that_is_not_a_count_and_a_unit_is_refused_by_what_was_typed() {
    // A UNIT IS REQUIRED: a bare number means minutes to one reader and
    // seconds to the next. The rest are the shapes `parse_count` already
    // refuses everywhere else in this crate, reaching the operator here as
    // a quotation of their own typing rather than as a silent coercion.
    for typed in ["30", "", "1d", "-5m", " 5m", "05m", "m", "ms", "2 h"] {
        assert_eq!(
            parse(typed),
            Err(format!(
                "pns: quiet duration {typed:?} is not <count><ms|s|m|h>"
            )),
            "typed: {typed:?}"
        );
    }
}

#[test]
fn a_refusal_names_the_field_it_was_parsing() {
    // ONE PARSER, EVERY DURATION FIELD, so the sentence cannot name one of
    // them: an operator sent to `quiet` over a mistyped flare has to guess
    // which argument the complaint is about.
    assert_eq!(
        parse_duration("flare", "30", RANGE),
        Err("pns: flare \"30\" is not <count><ms|s|m|h>".to_string())
    );
    assert_eq!(
        parse_duration("flare", "25h", RANGE),
        Err("pns: flare \"25h\" is outside 1s to 24h".to_string())
    );
}

#[test]
fn a_duration_outside_the_fields_own_range_is_refused_rather_than_clamped() {
    // Clamping either end would hand the operator a window they did not ask
    // for and believe they had set.
    for typed in ["0s", "25h", "1441m", "86401s", "9223372036854775807h"] {
        assert_eq!(
            parse(typed),
            Err(format!(
                "pns: quiet duration {typed:?} is outside 1s to 24h"
            )),
            "typed: {typed:?}"
        );
    }
    // And the two ends themselves are inside it.
    assert_eq!(parse("1s"), Ok(Duration::from_secs(1)));
    assert_eq!(parse("24h"), Ok(Duration::from_secs(86_400)));
}

#[test]
fn a_range_is_spelled_in_the_largest_unit_that_holds_it_whole() {
    // The bounds are the field's, so the sentence has to read back in the
    // units the operator would type rather than in the smallest unit held.
    let range = Duration::from_secs(90)..=Duration::from_secs(3_600);
    assert_eq!(
        parse_duration("delay", "3h", range),
        Err("pns: delay \"3h\" is outside 90s to 1h".to_string())
    );
}

#[test]
fn zero_spells_as_a_unit_the_parser_accepts_back() {
    // A ZERO-FLOORED RANGE must refuse in the same grammar it parses, or the
    // refusal names a lower bound the operator cannot type.
    let zero_floored = Duration::ZERO..=Duration::from_secs(86_400);
    assert_eq!(
        parse_duration("elapsed", "9223372036854775807h", zero_floored.clone()),
        Err("pns: elapsed \"9223372036854775807h\" is outside 0s to 24h".to_string())
    );
    assert_eq!(
        parse_duration("elapsed", "0s", zero_floored),
        Ok(Duration::ZERO)
    );
}

/// A stand-in range for these tests: the parser's own tests pin its
/// behavior, not one caller's policy bound.
const RANGE: RangeInclusive<Duration> = Duration::from_secs(1)..=Duration::from_secs(86_400);

fn parse(text: &str) -> Result<Duration, String> {
    parse_duration("quiet duration", text, RANGE)
}
