use super::{Edge, PresenceStatus, RawPresence, Unreadable, classify, idle_secs_from_ns};

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
