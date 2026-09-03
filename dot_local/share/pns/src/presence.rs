//! Where the operator is: the raw readings turned into the units the
//! arbitration compares.
//!
//! THE SPLIT INSIDE THIS MODULE MATTERS AS MUCH AS THE ONE BETWEEN THE MODULES.
//! Every function here is a function of its arguments; the readings themselves
//! sit behind the probe traits, so a test hands this module fixture bytes
//! instead of a live machine.
//!
//! THE FILE A ROOM READING ARRIVES IN IS `presence_file`'s, and the line
//! between them is syntax against meaning: that module says what a line SAYS
//! and this one says what it MEANS, because the writer and the routing are
//! what change them and those are two different changes.
//!
//! The arbitration those units feed lives in `surface`, and it lives there
//! ONCE: the engine deciding which channels fire and a harness gate deciding
//! whether a phone round trip fires at all both call it, because a second copy
//! is how the two would drift into disagreeing about where the operator is.

use crate::presence_file::RawPresence;

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

/// Where the operator is, as far as the bridge can say.
///
/// AN ENUM RATHER THAN AN `Option`, because every way of not knowing is a
/// different thing to go and fix, and the doctor line and the eventual
/// routing both need the reason an `Option` cannot carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceStatus {
    /// The room the operator is in, and how old its edge is. Motion reported
    /// NOW is age zero.
    Room { room: String, age_secs: u64 },
    /// A fresh poll that found motion in no watched room. NOT unknown: the
    /// bridge answered, and it answered "not there".
    Nowhere { poll_age_secs: u64 },
    /// No usable reading, and why.
    Unknown(Unreadable),
}

/// Why there is no reading. Every variant is a fail-CLOSED outcome: presence
/// narrows what the lights do and never widens it, so not knowing costs the
/// narrowing and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreadable {
    /// Nothing at the path, or nothing in it this parse accepts.
    NoReading,
    /// There was a line and no clock to age it against.
    NoClock,
    /// The last poll is older than the bound, so the writer stopped. A DEAD
    /// BRIDGE IS UNKNOWN, never present and never absent.
    Stale { poll_age_secs: u64 },
    /// An epoch newer than the clock. TWO-SIDED ON PURPOSE: reading a future
    /// epoch as the freshest input there is would let a wrong clock or a
    /// hand-written file pin the operator in a room for good.
    Future,
    /// The reported room is not one this config watches, or is excluded,
    /// which is not a claim about where the operator is.
    NotWatched,
}

/// What a reading MEANS: the whole policy, as a function of its arguments.
///
/// THE ORDER OF THE REFUSALS IS THE POINT. The future check runs before the
/// staleness arithmetic, because `now - poll_epoch` on a future epoch would
/// underflow into an enormous age (or, saturating, into a fresh-looking
/// zero); staleness runs before the room, because a room name off a poll
/// nobody refreshed is not evidence about now.
pub fn classify(
    raw: Option<RawPresence>,
    now: Option<u64>,
    stale_after_secs: u64,
    rooms: &[String],
    exclude: &[String],
) -> PresenceStatus {
    let Some(raw) = raw else {
        return PresenceStatus::Unknown(Unreadable::NoReading);
    };
    let Some(now) = now else {
        return PresenceStatus::Unknown(Unreadable::NoClock);
    };
    if raw.poll_epoch > now || raw.edge.as_ref().is_some_and(|edge| edge.epoch > now) {
        return PresenceStatus::Unknown(Unreadable::Future);
    }
    let poll_age_secs = now - raw.poll_epoch;
    if poll_age_secs >= stale_after_secs {
        return PresenceStatus::Unknown(Unreadable::Stale { poll_age_secs });
    }
    let Some(edge) = raw.edge else {
        return PresenceStatus::Nowhere { poll_age_secs };
    };
    if !rooms.contains(&edge.room) || exclude.contains(&edge.room) {
        return PresenceStatus::Unknown(Unreadable::NotWatched);
    }
    PresenceStatus::Room {
        // MOTION NOW IS AGE ZERO. The edge is when motion STARTED, so a room
        // still reporting motion after a long turn would otherwise age out of
        // freshness while the operator is standing in it.
        age_secs: if edge.motion { 0 } else { now - edge.epoch },
        room: edge.room,
    }
}

#[cfg(test)]
mod tests {
    use super::{PresenceStatus, Unreadable, classify, idle_secs_from_ns};
    use crate::presence_file::{Edge, RawPresence};

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

    // --- classify -----------------------------------------------------------

    /// The rooms the cases below share.
    fn watched() -> Vec<String> {
        vec!["3F - Studio".to_string(), "2F - Kitchen".to_string()]
    }

    /// A full line at these epochs, as `classify` takes it.
    fn reading(poll: u64, edge: u64, motion: bool, room: &str) -> Option<RawPresence> {
        Some(RawPresence {
            poll_epoch: poll,
            edge: Some(Edge {
                epoch: edge,
                motion,
                room: room.to_string(),
            }),
        })
    }

    #[test]
    fn a_fresh_poll_in_a_watched_room_names_that_room_and_its_edge_age() {
        assert_eq!(
            classify(
                reading(1000, 990, false, "3F - Studio"),
                Some(1002),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 12,
            }
        );
    }

    #[test]
    fn no_line_at_all_is_unknown_and_never_a_room() {
        assert_eq!(
            classify(None, Some(1000), 15, &watched(), &[]),
            PresenceStatus::Unknown(Unreadable::NoReading)
        );
    }

    #[test]
    fn a_clock_that_could_not_be_read_is_unknown_rather_than_epoch_zero() {
        // `unwrap_or(0)` would age every reading by fifty-five years.
        assert_eq!(
            classify(
                reading(1000, 990, false, "3F - Studio"),
                None,
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Unknown(Unreadable::NoClock)
        );
    }

    #[test]
    fn a_poll_at_the_stale_bound_is_unknown_and_one_second_under_it_is_not() {
        assert_eq!(
            classify(
                reading(1000, 1000, false, "3F - Studio"),
                Some(1015),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Unknown(Unreadable::Stale { poll_age_secs: 15 })
        );
        assert!(matches!(
            classify(
                reading(1000, 1000, false, "3F - Studio"),
                Some(1014),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Room { .. }
        ));
    }

    #[test]
    fn a_poll_epoch_newer_than_the_clock_is_unknown() {
        assert_eq!(
            classify(
                reading(1001, 1000, false, "3F - Studio"),
                Some(1000),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Unknown(Unreadable::Future)
        );
    }

    #[test]
    fn an_edge_epoch_newer_than_the_clock_is_unknown() {
        assert_eq!(
            classify(
                reading(1000, 1001, false, "3F - Studio"),
                Some(1000),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Unknown(Unreadable::Future)
        );
    }

    #[test]
    fn motion_reported_now_is_no_age_at_all_however_old_the_edge_is() {
        assert_eq!(
            classify(
                reading(1000, 1, true, "3F - Studio"),
                Some(1001),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 0,
            }
        );
    }

    #[test]
    fn a_room_the_config_never_listed_is_unknown() {
        assert_eq!(
            classify(
                reading(1000, 1000, true, "3F - Hallway"),
                Some(1001),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Unknown(Unreadable::NotWatched)
        );
    }

    #[test]
    fn an_excluded_room_is_unknown_even_though_it_is_listed() {
        let exclude = vec!["2F - Kitchen".to_string()];
        assert_eq!(
            classify(
                reading(1000, 1000, true, "2F - Kitchen"),
                Some(1001),
                15,
                &watched(),
                &exclude
            ),
            PresenceStatus::Unknown(Unreadable::NotWatched)
        );
    }

    #[test]
    fn a_fresh_poll_with_no_edge_is_nowhere_rather_than_unknown() {
        assert_eq!(
            classify(
                Some(RawPresence {
                    poll_epoch: 1000,
                    edge: None
                }),
                Some(1003),
                15,
                &watched(),
                &[]
            ),
            PresenceStatus::Nowhere { poll_age_secs: 3 }
        );
    }
}
