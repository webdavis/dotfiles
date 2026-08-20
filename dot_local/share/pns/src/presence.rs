//! Where the operator is: the raw readings turned into the units the
//! arbitration compares.
//!
//! THE SPLIT INSIDE THIS MODULE MATTERS AS MUCH AS THE ONE BETWEEN THE MODULES.
//! Every function here is a function of its arguments; the readings themselves
//! sit behind the probe traits, so a test hands this module fixture bytes
//! instead of a live machine.
//!
//! The arbitration those units feed lives in `surface`, and it lives there
//! ONCE: the engine deciding which channels fire and a harness gate deciding
//! whether a phone round trip fires at all both call it, because a second copy
//! is how the two would drift into disagreeing about where the operator is.

/// The unit the idle counter is read in.
const NANOSECONDS_PER_SEC: u64 = 1_000_000_000;

/// Seconds since the last human input, read from a nanosecond counter, or
/// `None` when that cannot be read.
///
/// None is the unknown verdict, and the phone rule reads unknown as away: a
/// garbled probe line must never coerce to 0, which reads as "actively typing"
/// and silently drops the push.
pub fn idle_secs_from_ns(idle_nanoseconds: &str) -> Option<u64> {
    crate::parse_count(idle_nanoseconds).map(|nanoseconds| nanoseconds / NANOSECONDS_PER_SEC)
}

#[cfg(test)]
mod tests {
    use super::idle_secs_from_ns;

    // --- idle_secs_from_ns -------------------------------------------------

    #[test]
    fn a_nanosecond_counter_becomes_whole_seconds() {
        assert_eq!(idle_secs_from_ns("5000000000"), Some(5));
    }

    #[test]
    fn a_partial_second_truncates_rather_than_rounding_up() {
        assert_eq!(idle_secs_from_ns("1999999999"), Some(1));
        assert_eq!(idle_secs_from_ns("0"), Some(0));
    }

    #[test]
    fn an_empty_reading_is_unknown_rather_than_zero_seconds_idle() {
        // Zero would read as "actively typing" and silently drop the push.
        assert_eq!(idle_secs_from_ns(""), None);
    }

    #[test]
    fn a_garbled_reading_is_unknown() {
        assert_eq!(idle_secs_from_ns("HIDIdleTime"), None);
        assert_eq!(idle_secs_from_ns("5000000000 "), None);
    }
}
