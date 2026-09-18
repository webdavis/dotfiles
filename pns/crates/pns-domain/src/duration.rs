//! How long, typed as a count and a unit, for every field that takes one.

use std::ops::RangeInclusive;
use std::time::Duration;

/// A duration, from `<count><s|m|h>`, inside the range the field allows.
///
/// A UNIT IS REQUIRED, and the count goes through the crate's one numeric
/// gate, so every shape `parse_count` refuses elsewhere is refused here too.
/// A bare number is not accepted at either end: it means minutes to one reader
/// and seconds to the next.
///
/// THE FIELD NAMES ITSELF in both refusals, because one parser now serves
/// every duration in pns and a message that named one of them would send the
/// operator to the wrong argument.
///
/// THE RANGE IS THE FIELD'S OWN, since what is sane for a mute is not what is
/// sane for a flare, and refused rather than clamped: a value silently moved
/// is a window the operator believes they set.
pub fn parse_duration(
    field: &str,
    text: &str,
    range: RangeInclusive<Duration>,
) -> Result<Duration, String> {
    for (unit, millis) in UNITS {
        if let Some(digits) = text.strip_suffix(unit)
            && let Some(count) = crate::count::parse_count(digits)
        {
            // SATURATING, so the range below is what refuses a count too large
            // to multiply rather than an overflow deciding it.
            let total = Duration::from_millis(count.saturating_mul(millis));
            if !range.contains(&total) {
                let (low, high) = (spelled(*range.start()), spelled(*range.end()));
                return Err(format!("pns: {field} {text:?} is outside {low} to {high}"));
            }
            return Ok(total);
        }
    }
    Err(format!("pns: {field} {text:?} is not <count><s|m|h>"))
}

/// A duration written back in the largest unit that holds it whole, which is
/// how a range reads in a refusal the operator has to act on, and how a
/// duration goes back out on the wire.
pub fn spelled(duration: Duration) -> String {
    let millis = duration.as_millis();
    // ZERO SPELLS AS "0s", the smallest unit the parser accepts: the `ms`
    // fallback below exists for sub-second values it cannot produce, and
    // zero is not one of those.
    if millis == 0 {
        return "0s".to_string();
    }
    for (unit, step) in UNITS.iter().rev() {
        let step = u128::from(*step);
        if millis >= step && millis.is_multiple_of(step) {
            return format!("{}{unit}", millis / step);
        }
    }
    format!("{millis}ms")
}

/// The units a duration may be typed in, and what each is worth in
/// milliseconds.
const UNITS: [(&str, u64); 3] = [("s", 1_000), ("m", 60_000), ("h", 3_600_000)];

#[cfg(test)]
mod tests;
