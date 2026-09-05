//! One CLIP instant (`2026-09-03T17:20:09.413Z`) as the second it names.
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

/// A CLIP instant as the second it names.
///
/// STRICT ABOUT THE SHAPE, in `parse_count`'s spirit: a field this does not
/// recognise is `None`, so the room contributes no edge at all rather than an
/// edge at a second nobody meant. Milliseconds are DROPPED rather than
/// rounded, because the reading they feed is aged in whole seconds.
pub fn epoch_from_utc(stamp: &str) -> Option<u64> {
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
    // rather than folded into a plausible-looking second.
    if !((1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 61)
    {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    // A PRE-EPOCH INSTANT IS REFUSED rather than wrapped: every reading this
    // feeds is aged against a Unix second.
    u64::try_from(seconds).ok()
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
    use super::epoch_from_utc;

    #[test]
    fn an_instant_becomes_the_second_it_names_and_its_milliseconds_are_dropped() {
        assert_eq!(epoch_from_utc("2026-09-03T17:20:09Z"), Some(1_788_456_009));
        assert_eq!(
            epoch_from_utc("2026-09-03T17:20:09.413Z"),
            Some(1_788_456_009)
        );
        assert_eq!(epoch_from_utc("1970-01-01T00:00:00Z"), Some(0));
        // A leap day and the day the shifted-era arithmetic starts its year.
        assert_eq!(epoch_from_utc("2024-02-29T12:34:56Z"), Some(1_709_210_096));
        assert_eq!(epoch_from_utc("2000-03-01T00:00:00Z"), Some(951_868_800));
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
            assert_eq!(epoch_from_utc(stamp), None, "{stamp:?} was accepted");
        }
    }
}
