//! The operator mute: a typed, timed instruction to stop decorating.

use std::ops::RangeInclusive;
use std::time::Duration;

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
             nothing is muted, clear it with pns mute off"
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

/// What `pns mute` says, for every state the predicate can be in.
///
/// THE VERDICT IS `is_muted`'S, never re-derived here: one property read by
/// two readers that each decide it is how a report and a behavior come to
/// disagree about whether a mute is on.
pub fn status_line(expiry: Option<u64>, now: Option<u64>) -> String {
    match (is_muted(expiry, now), expiry, now) {
        (true, Some(expiry), Some(now)) => {
            let minutes = minutes_left(expiry, Some(now));
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!("pns: muted for another {minutes} {unit}")
        }
        _ => "pns: not muted".to_string(),
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

/// How long a mute may last: the range `duration::parse_duration` holds
/// `pns mute` and `pns lights mute` to, since one spelling of "how long"
/// cannot have two sets of bounds.
///
/// A ZERO would write a state file born expired. A DAY is the ceiling, and
/// refused rather than clamped past it: a mute the operator forgets is a
/// notification system that has silently stopped working, and a mistyped
/// `900h` is that by another route.
pub const MUTE_RANGE: RangeInclusive<Duration> =
    Duration::from_secs(1)..=Duration::from_secs(24 * 60 * 60);

pub mod calendar;

#[cfg(test)]
mod tests;
