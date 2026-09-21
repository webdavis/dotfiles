//! RFC 3339 times, both directions, in epoch seconds.
//!
//! HAND-WRITTEN BECAUSE THE WORKSPACE CARRIES NO TIME CRATE, and the grammar
//! this needs is one shape: `YYYY-MM-DDTHH:MM:SS`, an optional fraction, and
//! either `Z` or `±HH:MM`. The civil-date arithmetic is the standard
//! days-from-civil algorithm, proleptic Gregorian.

/// One epoch second as `YYYY-MM-DDTHH:MM:SSZ`, which is what the freeBusy
/// request's window bounds are written as.
pub(super) fn format_rfc3339(epoch: u64) -> String {
    let (year, month, day) = civil_from_days((epoch / 86_400) as i64);
    let second_of_day = epoch % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        second_of_day / 3_600,
        (second_of_day / 60) % 60,
        second_of_day % 60
    )
}

/// One RFC 3339 time as epoch seconds, or `None` for anything that is not
/// one.
///
/// A TIME BEFORE THE EPOCH IS `None` TOO: the events this reads are the next
/// hour's, so a 1969 bound is a malformed answer rather than a meeting.
pub(super) fn parse_rfc3339(stated: &str) -> Option<u64> {
    let (date, rest) = stated.split_once('T')?;
    let (year, month, day) = parse_date(date)?;
    let (clock, offset_secs) = split_offset(rest)?;
    let (hour, minute, second) = parse_clock(clock)?;
    let civil = days_from_civil(year, month, day) * 86_400
        + i64::from(hour) * 3_600
        + i64::from(minute) * 60
        + i64::from(second);
    u64::try_from(civil - offset_secs).ok()
}

fn parse_date(date: &str) -> Option<(i64, u32, u32)> {
    let mut parts = date.split('-');
    let year: i64 = fixed_digits(parts.next()?, 4)?;
    let month: u32 = fixed_digits(parts.next()?, 2)?;
    let day: u32 = fixed_digits(parts.next()?, 2)?;
    (parts.next().is_none() && (1..=12).contains(&month) && (1..=31).contains(&day))
        .then_some((year, month, day))
}

/// The clock, with the fraction dropped: a busy interval is muted to the
/// second, so a millisecond changes nothing this decides.
fn parse_clock(clock: &str) -> Option<(u32, u32, u32)> {
    let clock = clock.split_once('.').map_or(clock, |(whole, _)| whole);
    let mut parts = clock.split(':');
    let hour: u32 = fixed_digits(parts.next()?, 2)?;
    let minute: u32 = fixed_digits(parts.next()?, 2)?;
    let second: u32 = fixed_digits(parts.next()?, 2)?;
    // A LEAP SECOND (`60`) IS ADMITTED and lands on the following second.
    (parts.next().is_none() && hour < 24 && minute < 60 && second <= 60)
        .then_some((hour, minute, second))
}

/// The clock and how many seconds its zone is ahead of UTC.
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(clock) = rest.strip_suffix('Z') {
        return Some((clock, 0));
    }
    let sign_at = rest.rfind(['+', '-'])?;
    let (clock, offset) = rest.split_at(sign_at);
    let (sign, offset) = offset.split_at(1);
    let (hours, minutes) = offset.split_once(':')?;
    let hours: i64 = fixed_digits(hours, 2)?;
    let minutes: i64 = fixed_digits(minutes, 2)?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    let seconds = hours * 3_600 + minutes * 60;
    Some((clock, if sign == "-" { -seconds } else { seconds }))
}

/// A fixed-width run of ASCII digits, as a number.
///
/// WIDTH IS CHECKED BECAUSE `parse` IS NOT ENOUGH: `"1"` parses as a year and
/// `"+5"` parses as a month, and neither is an RFC 3339 field.
fn fixed_digits<T: std::str::FromStr>(text: &str, width: usize) -> Option<T> {
    (text.len() == width && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse: a civil date out of days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = (shifted_month + if shifted_month < 10 { 3 } else { -9 }) as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests;
