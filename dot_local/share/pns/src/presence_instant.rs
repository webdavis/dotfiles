//! One CLIP instant (`2026-09-03T17:20:09.413Z`) as the second it names and
//! the fraction inside it.
//!
//! ITS OWN MODULE, and the seam is the same one `presence_file` cuts against
//! `presence`: this is a TIMESTAMP GRAMMAR and nothing else, where
//! `presence_hue` is the join of two bridge listings into one reading. They
//! change for different reasons, a new field shape against a new bridge
//! behavior, and a parse that also joined would be edited by both.
//!
//! PURE ARITHMETIC, NOT A SYSTEM CALL, which is the one place this crate reads
//! a clock field without libc: turning a UTC civil time into an epoch second
//! involves no zone database, no daylight-saving transition and no leap
//! second. The other direction (`system::utc_timestamp`) asks libc because it
//! is handed an epoch and a zone question; this one is handed the answer.

/// A CLIP instant as the second it names AND the nanoseconds inside it.
///
/// TWO NUMBERS BECAUSE ONE SECOND HOLDS TWO EDGES. The bridge's `changed`
/// carries milliseconds and the state file carries whole seconds, so the
/// COMPARISON that picks the newest edge needs precision the FORMAT throws
/// away. Reducing before comparing made two edges 800ms apart compare equal
/// and left the tie to be broken by the order the bridge listed its rooms in.
/// The caller reduces the edge it chose, and not the ones it was choosing
/// between.
///
/// STRICT ABOUT THE SHAPE, in `parse_count`'s spirit: a field this does not
/// recognise is `None`, so the room contributes no edge at all rather than an
/// edge at a second nobody meant.
pub fn instant_from_utc(stamp: &str) -> Option<(u64, u32)> {
    if stamp.len() < 20 || !stamp.is_ascii() {
        return None;
    }
    let bytes = stamp.as_bytes();
    if [bytes[4], bytes[7], bytes[10], bytes[13], bytes[16]] != *b"--T::" {
        return None;
    }
    // THE FRACTION IS OPTIONAL AND EVERYTHING ELSE IS NOT: `Z` alone, or a
    // decimal point, digits and then `Z`. An offset (`+02:00`) is refused
    // rather than read as UTC, which would be an hour's error stated
    // confidently.
    let tail = &stamp[19..];
    let fraction = tail.strip_suffix('Z')?;
    if !(fraction.is_empty() || (fraction.starts_with('.') && digits(&fraction[1..]).is_some())) {
        return None;
    }
    let year = digits(&stamp[0..4])?;
    let month = digits(&stamp[5..7])?;
    let day = digits(&stamp[8..10])?;
    let hour = digits(&stamp[11..13])?;
    let minute = digits(&stamp[14..16])?;
    let second = digits(&stamp[17..19])?;
    // Range-checked before the arithmetic, so a garbled field is refused
    // rather than folded into a plausible-looking second. The month is checked
    // FIRST because the day's own bound is read out of it.
    if !((1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
        && hour < 24
        && minute < 60
        // BELOW SIXTY, because this arithmetic knows nothing of leap seconds:
        // `60` was folded into the following minute, a second the bridge never
        // named.
        && second < 60)
    {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    // A PRE-EPOCH INSTANT IS REFUSED rather than wrapped: every reading this
    // feeds is aged against a Unix second.
    Some((u64::try_from(seconds).ok()?, nanos_of(fraction)))
}

/// The fraction after the seconds field as whole nanoseconds.
///
/// TRUNCATED AT NINE DIGITS AND PADDED BELOW THEM, so `.4`, `.400` and
/// `.400000000` are one instant and a bridge that grew a tenth digit is read
/// rather than refused. The caller has already checked the text is a `.`
/// followed by digits, or empty.
fn nanos_of(fraction: &str) -> u32 {
    let digits = fraction.as_bytes().get(1..).unwrap_or_default();
    (0..9).fold(0, |nanos, place| {
        nanos * 10 + u32::from(digits.get(place).map_or(0, |digit| digit - b'0'))
    })
}

/// How many days that month has, Gregorian, leap years included.
///
/// IT EXISTS BECAUSE THE ARITHMETIC BELOW NORMALISES RATHER THAN REFUSING:
/// `days_from_civil` reads 2026-02-31 as March 3, an instant three days NEWER
/// than the one the bridge named, and newer is exactly what the caller
/// compares edges by. A date nobody can have is no edge at all.
///
/// The month is the caller's to bound, so `_` here is the thirty-one-day set
/// and nothing else can reach it.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to this civil date, by Howard Hinnant's
/// `days_from_civil`. Signed on purpose: the shifted-era arithmetic runs
/// negative for every date before 1970-03-01, and the caller refuses the
/// result rather than wrapping it.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    // MARCH IS THE START OF THE YEAR here, which is what puts the leap day at
    // the END of it and lets one expression cover every year length.
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// A run of plain digits as a number, or `None`. `parse_count` cannot serve
/// here: it refuses a leading zero by design, and every field of a timestamp
/// is zero-padded.
fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::instant_from_utc;

    #[test]
    fn an_instant_becomes_the_second_it_names_and_the_fraction_inside_it() {
        assert_eq!(
            instant_from_utc("2026-09-03T17:20:09Z"),
            Some((1_788_456_009, 0))
        );
        assert_eq!(
            instant_from_utc("2026-09-03T17:20:09.413Z"),
            Some((1_788_456_009, 413_000_000))
        );
        assert_eq!(instant_from_utc("1970-01-01T00:00:00Z"), Some((0, 0)));
        // A leap day and the day the shifted-era arithmetic starts its year.
        assert_eq!(
            instant_from_utc("2024-02-29T12:34:56Z"),
            Some((1_709_210_096, 0))
        );
        assert_eq!(
            instant_from_utc("2000-03-01T00:00:00Z"),
            Some((951_868_800, 0))
        );
    }

    #[test]
    fn a_fraction_is_padded_below_nine_digits_and_truncated_above_them() {
        // ONE INSTANT, THREE SPELLINGS, so a bridge that trims its trailing
        // zeroes does not compare as an earlier edge than one that does not.
        for stamp in [
            "2026-09-03T17:20:09.4Z",
            "2026-09-03T17:20:09.400Z",
            "2026-09-03T17:20:09.400000000Z",
        ] {
            assert_eq!(
                instant_from_utc(stamp),
                Some((1_788_456_009, 400_000_000)),
                "{stamp:?} is the same instant as the others"
            );
        }
        // Past a nanosecond the extra digits are dropped rather than refused.
        assert_eq!(
            instant_from_utc("2026-09-03T17:20:09.4000000009Z"),
            Some((1_788_456_009, 400_000_000))
        );
    }

    #[test]
    fn a_day_the_month_does_not_have_is_refused_rather_than_rolled_forward() {
        // THE ARITHMETIC BELOW NORMALISES, which is the danger: 2026-02-31
        // came back as March 3, an instant three days NEWER than the one the
        // bridge named, and newer is exactly what the caller compares edges
        // by. So an impossible date could outrank a real one and name the
        // wrong room.
        for stamp in [
            "2026-02-29T17:20:09Z",
            "2026-02-31T17:20:09Z",
            "2024-02-30T17:20:09Z",
            "2100-02-29T17:20:09Z",
            "2026-04-31T17:20:09Z",
            "2026-06-31T17:20:09Z",
            "2026-09-31T17:20:09Z",
            "2026-11-31T17:20:09Z",
            "2026-01-00T17:20:09Z",
        ] {
            assert_eq!(instant_from_utc(stamp), None, "{stamp:?} was accepted");
        }
        // AND THE DAYS THOSE MONTHS DO HAVE STILL READ, leap years included by
        // both rules that make one: divisible by four, and the century that is
        // divisible by four hundred.
        for stamp in [
            "2024-02-29T17:20:09Z",
            "2000-02-29T17:20:09Z",
            "2026-02-28T17:20:09Z",
            "2026-04-30T17:20:09Z",
            "2026-01-31T17:20:09Z",
        ] {
            assert!(instant_from_utc(stamp).is_some(), "{stamp:?} was refused");
        }
    }

    #[test]
    fn a_sixtieth_second_is_refused_because_leap_seconds_are_unsupported() {
        // The module says leap seconds are not supported and the range check
        // admitted `60` anyway, so a second the arithmetic below cannot mean
        // was folded into the following minute.
        assert_eq!(instant_from_utc("2026-09-03T17:20:60Z"), None);
        assert_eq!(
            instant_from_utc("2026-09-03T17:20:59Z"),
            Some((1_788_456_059, 0))
        );
    }

    #[test]
    fn an_instant_this_does_not_recognise_contributes_no_edge() {
        for stamp in [
            "",
            "2026-09-03",
            "2026-09-03T17:20:09",
            "2026-09-03 17:20:09Z",
            "2026-09-03T17:20:09+02:00",
            "2026-13-03T17:20:09Z",
            "2026-09-32T17:20:09Z",
            "2026-09-03T24:20:09Z",
            "2026-09-03T17:60:09Z",
            "20xx-09-03T17:20:09Z",
            "1969-12-31T23:59:59Z",
            "2026-09-03T17:20:09.Z",
            "2026-09-03T17:20:09.413",
        ] {
            assert_eq!(instant_from_utc(stamp), None, "{stamp:?} was accepted");
        }
    }
}
