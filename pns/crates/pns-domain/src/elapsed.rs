use crate::EventArgs;
use std::ops::RangeInclusive;
use std::time::Duration;

/// How long a producer may say its work ran.
///
/// A ZERO IS ALLOWED, because a shell reporting a command that finished inside
/// a second is telling the truth. THE CEILING IS WIDE ON PURPOSE, wider than
/// any window pns decides anything with: this is a measurement rather than a
/// policy value, and a soak that really did run for a week must not be refused
/// while a wrapped or garbage count still is.
pub const RANGE: RangeInclusive<Duration> = Duration::ZERO..=Duration::from_secs(30 * 24 * 60 * 60);

pub fn elapsed_event(mut event: EventArgs, seconds: u64) -> Option<EventArgs> {
    if seconds < 30 {
        return None;
    }
    event.long_running = seconds >= 300;
    event.detail = if event.detail.is_empty() {
        format!("{seconds}s")
    } else {
        format!("{} ({seconds}s)", event.detail)
    };
    Some(event)
}
