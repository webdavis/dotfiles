//! The operator mute: a typed, timed instruction to stop decorating.

/// A duration in seconds, from `<count><s|m|h>`.
///
/// A UNIT IS REQUIRED, and the count goes through the crate's one numeric
/// gate, so every shape `parse_count` refuses elsewhere is refused here too.
/// A bare number is not accepted at either end: it means minutes to one reader
/// and seconds to the next.
pub fn parse_duration(text: &str) -> Result<u64, String> {
    for (unit, seconds) in UNITS {
        if let Some(digits) = text.strip_suffix(unit)
            && let Some(count) = crate::parse_count(digits)
        {
            // SATURATING, so the ceiling below is what refuses a count too
            // large to multiply rather than an overflow deciding it.
            let total = count.saturating_mul(seconds);
            if !(MIN_SECONDS..=MAX_SECONDS).contains(&total) {
                return Err(format!("pns: quiet duration {text:?} is outside 1s to 24h"));
            }
            return Ok(total);
        }
    }
    Err(format!(
        "pns: quiet duration {text:?} is not <count><s|m|h>"
    ))
}

/// The epoch second a mute ends, out of the state file's contents.
pub fn expiry_from_state(contents: &str) -> Result<u64, String> {
    let held = contents.trim();
    crate::parse_count(held).ok_or_else(|| {
        format!(
            "pns: state error (quiet-until is {held:?}, not an expiry time); \
             nothing is muted, clear it with pns quiet off"
        )
    })
}

/// Whether a mute is on: an expiry, judged against the run's own clock.
///
/// FAIL OPEN on everything unreadable, which is deliberately the OPPOSITE
/// direction to `hue::quiet_now` in the same feature family. That window
/// failing closed costs one flash of a lamp; this failing closed costs every
/// notification, including the card for a tool call the operator is blocked
/// on, with no expiry on it and no way for them to discover it. A mute nobody
/// can see is the dangerous state.
///
/// HALF OPEN: the expiry second itself is already over, so a mute ends when it
/// says it does.
pub fn is_muted(expiry: Option<u64>, now: Option<u64>) -> bool {
    match (expiry, now) {
        (Some(expiry), Some(now)) => now < expiry,
        _ => false,
    }
}

/// What `pns quiet` says, for every state the predicate can be in.
///
/// THE VERDICT IS `is_muted`'S, never re-derived here: one property read by
/// two readers that each decide it is how a report and a behavior come to
/// disagree about whether a mute is on.
pub fn status_line(expiry: Option<u64>, now: Option<u64>) -> String {
    match (is_muted(expiry, now), expiry, now) {
        (true, Some(expiry), Some(now)) => {
            // ROUNDED UP, so a mute with forty seconds left never reports the
            // zero minutes that reads as off.
            let minutes = expiry.saturating_sub(now).div_ceil(60);
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!("pns: quiet for another {minutes} {unit}")
        }
        _ => "pns: not quiet".to_string(),
    }
}

/// The units a mute may be typed in, and what each is worth in seconds.
const UNITS: [(&str, u64); 3] = [("s", 1), ("m", 60), ("h", 3_600)];

/// A mute spans real time: a zero would write a state file born expired.
const MIN_SECONDS: u64 = 1;

/// A DAY, and refused rather than clamped past it. A mute the operator forgets
/// is a notification system that has silently stopped working, and a mistyped
/// `900h` is that by another route.
const MAX_SECONDS: u64 = 24 * 60 * 60;

#[cfg(test)]
mod tests {
    use super::{expiry_from_state, is_muted, parse_duration, status_line};

    #[test]
    fn a_count_and_its_unit_are_that_many_seconds() {
        assert_eq!(parse_duration("30m"), Ok(1_800));
        assert_eq!(parse_duration("45s"), Ok(45));
        assert_eq!(parse_duration("2h"), Ok(7_200));
    }

    #[test]
    fn a_duration_that_is_not_a_count_and_a_unit_is_refused_by_what_was_typed() {
        // A UNIT IS REQUIRED: a bare number means minutes to one reader and
        // seconds to the next. The rest are the shapes `parse_count` already
        // refuses everywhere else in this crate, reaching the operator here as
        // a quotation of their own typing rather than as a silent coercion.
        for typed in ["30", "", "1d", "-5m", " 5m", "05m", "m", "2 h"] {
            assert_eq!(
                parse_duration(typed),
                Err(format!(
                    "pns: quiet duration {typed:?} is not <count><s|m|h>"
                )),
                "typed: {typed:?}"
            );
        }
    }

    #[test]
    fn a_duration_outside_the_permitted_range_is_refused_rather_than_clamped() {
        // A ZERO would be a state file born already expired, and the ceiling
        // is the indefinite mute by another route: a fat-fingered `900h` the
        // operator never notices. Clamping either end would hand them a window
        // they did not ask for and believe they had set.
        for typed in [
            "0s",
            "0m",
            "0h",
            "25h",
            "1441m",
            "86401s",
            "9223372036854775807h",
        ] {
            assert_eq!(
                parse_duration(typed),
                Err(format!(
                    "pns: quiet duration {typed:?} is outside 1s to 24h"
                )),
                "typed: {typed:?}"
            );
        }
        // And the two ends themselves are inside it.
        assert_eq!(parse_duration("1s"), Ok(1));
        assert_eq!(parse_duration("24h"), Ok(86_400));
    }

    #[test]
    fn a_state_file_holding_one_epoch_second_is_that_expiry() {
        // The trailing newline is how the file is published, so reading it
        // back is the round trip and not a lenient extra.
        assert_eq!(expiry_from_state("1800000000\n"), Ok(1_800_000_000));
        assert_eq!(expiry_from_state("1800000000"), Ok(1_800_000_000));
    }

    #[test]
    fn a_state_file_holding_anything_else_is_a_complaint_naming_what_it_holds() {
        // THE FILE'S OWN CONTENT is in the sentence, because the operator has
        // to find it to fix it and a complaint about an unnamed value sends
        // them looking. Two lines is one epoch second appended to another,
        // which is what a second writer racing this one would leave.
        for (contents, named) in [
            ("later\n", "\"later\""),
            ("\n", "\"\""),
            ("", "\"\""),
            ("1800000000\n1800000060\n", "\"1800000000\\n1800000060\""),
            ("-5", "\"-5\""),
        ] {
            assert_eq!(
                expiry_from_state(contents),
                Err(format!(
                    "pns: state error (quiet-until is {named}, not an expiry time); \
                     nothing is muted, clear it with pns quiet off"
                )),
                "contents: {contents:?}"
            );
        }
    }

    #[test]
    fn the_mute_ends_at_the_second_it_says_and_not_one_later() {
        // HALF OPEN, and THE BOUNDARY SECOND ITSELF is the assertion: a `<=`
        // here is an off-by-one nobody sees, because both neighbours agree
        // under either spelling.
        assert!(is_muted(Some(1_000), Some(999)));
        assert!(!is_muted(Some(1_000), Some(1_000)));
        assert!(!is_muted(Some(1_000), Some(1_001)));
    }

    #[test]
    fn nothing_readable_is_not_muted_which_is_the_opposite_of_the_lights_window() {
        // FAIL OPEN, deliberately the other way round from `hue::quiet_now`.
        // A window failing closed costs one flash of a lamp; a mute failing
        // closed costs every card, including one the operator is blocked on,
        // with no expiry and no way for them to discover it.
        assert!(!is_muted(None, Some(1_000)));
        assert!(!is_muted(Some(1_000_000), None));
        assert!(!is_muted(None, None));
    }

    #[test]
    fn the_report_counts_whole_minutes_up_so_a_live_mute_never_reads_as_zero() {
        // ROUNDED UP: a mute with forty seconds left is still on, and "0
        // minutes" reads as off. No wall-clock time is rendered anywhere,
        // which keeps the local zone out of a report that has no behavior
        // resting on it.
        let now = 1_000;
        assert_eq!(
            status_line(Some(now + 1_620), Some(now)),
            "pns: quiet for another 27 minutes"
        );
        assert_eq!(
            status_line(Some(now + 40), Some(now)),
            "pns: quiet for another 1 minute"
        );
        assert_eq!(
            status_line(Some(now + 60), Some(now)),
            "pns: quiet for another 1 minute"
        );
        assert_eq!(
            status_line(Some(now + 61), Some(now)),
            "pns: quiet for another 2 minutes"
        );
    }

    #[test]
    fn the_report_says_not_quiet_for_every_state_the_predicate_calls_quiet() {
        // ONE VERDICT, TWO READERS is this project's most repeated finding, so
        // the report is pinned against the SAME rows `is_muted` answers false
        // to: an expired mute, an absent one, and an unreadable clock.
        for (expiry, now) in [
            (Some(1_000), Some(1_000)),
            (Some(1_000), Some(9_999)),
            (None, Some(1_000)),
            (Some(1_000), None),
            (None, None),
        ] {
            assert!(!is_muted(expiry, now), "case: {expiry:?} {now:?}");
            assert_eq!(
                status_line(expiry, now),
                "pns: not quiet",
                "case: {expiry:?} {now:?}"
            );
        }
    }
}
