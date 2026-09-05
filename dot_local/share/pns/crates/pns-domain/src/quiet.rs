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
            && let Some(count) = crate::count::parse_count(digits)
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
///
/// EXACTLY ONE EPOCH LINE, and the only leniency is the ONE trailing newline
/// the publish itself writes. A `trim()` here read `" 9223372036854775807\n"`
/// as a live mute with 153722867251113165 minutes left on it. Padding is not
/// something this ever wrote, so a file carrying it was edited by something
/// else, and the fail-open rule is that anything but one plain epoch line
/// complains rather than mutes.
pub fn expiry_from_state(contents: &str) -> Result<u64, String> {
    let held = contents.strip_suffix('\n').unwrap_or(contents);
    crate::count::parse_count(held).ok_or_else(|| {
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
            let minutes = minutes_left(expiry, Some(now));
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!("pns: quiet for another {minutes} {unit}")
        }
        _ => "pns: not quiet".to_string(),
    }
}

/// How many whole minutes a mute has left.
///
/// ROUNDED UP, so a mute with forty seconds left never reports the zero minutes
/// that reads as off. ONE ROUNDING RULE for both reports that quote one, since
/// two would disagree at exactly the second an operator is looking.
pub fn minutes_left(expiry: u64, now: Option<u64>) -> u64 {
    expiry.saturating_sub(now.unwrap_or(expiry)).div_ceil(60)
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
mod tests;
