use super::{Full, Snapshot, chosen};
use crate::home::identity::DeviceKey;
use crate::home::reading::HomePresence;
use crate::presence::status::{PresenceStatus, Unreadable};

/// The operator's own snapshot, with the desk cold and the phone home.
fn snapshot(status: PresenceStatus) -> Snapshot {
    Snapshot {
        status,
        desk_idle_secs: None,
        screen_locked: Some(false),
        home: HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        },
        desk_room: Some("3F - Studio".to_string()),
        desk_stale_after_secs: 120,
        now: Some(1_700_000_000),
    }
}

/// A fresh reading naming one room.
fn in_room(room: &str) -> PresenceStatus {
    PresenceStatus::Room {
        room: room.to_string(),
        age_secs: 0,
    }
}

/// The study, as the answer this module returns.
fn studio() -> Result<String, Full> {
    Ok("3F - Studio".to_string())
}

#[test]
fn a_fresh_reading_with_no_desk_to_weigh_it_against_names_its_own_room() {
    assert_eq!(
        chosen(&snapshot(in_room("2F - Kitchen"))),
        Ok("2F - Kitchen".to_string())
    );
}

#[test]
fn a_poll_that_found_nobody_answers_nothing_and_keeps_its_own_reason() {
    assert_eq!(
        chosen(&snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })),
        Err(Full::Nowhere)
    );
}

#[test]
fn every_unreadable_reading_answers_nothing_and_keeps_its_own_reason() {
    // FIVE DIFFERENT THINGS TO GO AND FIX, and the variant is what carries
    // which: collapsed to a single "unknown" the operator cannot tell a
    // daemon that stopped from a room this config never watches. The five
    // WORDINGS are pinned where they are written down, in
    // `presence_journal::every_way_a_routing_can_be_left_whole_names_its_own_reason`.
    for reason in [
        Unreadable::NoReading,
        Unreadable::NoClock,
        Unreadable::Stale { poll_age_secs: 90 },
        Unreadable::Future,
        Unreadable::NotWatched,
    ] {
        assert_eq!(
            chosen(&snapshot(PresenceStatus::Unknown(reason))),
            Err(Full::Unknown(reason)),
            "{reason:?}"
        );
    }
}

#[test]
fn a_desk_still_being_typed_at_beats_motion_of_the_same_age() {
    // A cat crossing the kitchen must not move the lamps off a keyboard
    // somebody is typing on: nobody is in two rooms at once, and only one
    // of these two readings is made by a human hand.
    let taken = Snapshot {
        desk_idle_secs: Some(0),
        ..snapshot(in_room("2F - Kitchen"))
    };
    assert_eq!(chosen(&taken), studio());
}

#[test]
fn a_warm_desk_and_newer_motion_in_another_room_answer_nothing_at_all() {
    // THE BRIDGE REPORTS A STILL-OCCUPIED ROOM AS AGE ZERO, so three
    // seconds after a keystroke the kitchen is "newer" by the clock while
    // the operator is still at the keyboard. Two live readings that cannot
    // both be them, and nothing here can say which is: the whole house
    // signals rather than the wrong half of it.
    for desk_idle_secs in [1, 3, 60, 119] {
        let taken = Snapshot {
            desk_idle_secs: Some(desk_idle_secs),
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(
            chosen(&taken),
            Err(Full::Ambiguous {
                desk: "3F - Studio".to_string(),
                motion: "2F - Kitchen".to_string(),
            }),
            "at {desk_idle_secs}s idle the kitchen edge is somebody, and not provably them"
        );
    }
}

#[test]
fn a_warm_desk_and_newer_motion_in_its_own_room_agree_and_answer_that_room() {
    // The two readings are not in conflict at all: somebody moved in the
    // room the keyboard is in. Left ambiguous, the one case where both
    // sensors say the same thing would narrow nothing.
    let taken = Snapshot {
        desk_idle_secs: Some(3),
        ..snapshot(in_room("3F - Studio"))
    };
    assert_eq!(chosen(&taken), studio());
}

#[test]
fn a_desk_past_its_bound_stops_competing_and_motion_answers_alone() {
    // Past the bound the keyboard says nothing about which room a body is
    // standing in, so there are no longer two readings to weigh.
    for desk_idle_secs in [120, 121, 300] {
        let taken = Snapshot {
            desk_idle_secs: Some(desk_idle_secs),
            ..snapshot(in_room("2F - Kitchen"))
        };
        assert_eq!(
            chosen(&taken),
            Ok("2F - Kitchen".to_string()),
            "at {desk_idle_secs}s idle the desk has no claim left"
        );
    }
}

#[test]
fn a_desk_nobody_has_touched_for_longer_than_the_bound_speaks_for_nothing() {
    // Past the bound the desk stops competing at all, so a reading that
    // says only "nowhere" no longer parks every signal in the study.
    let cold = Snapshot {
        desk_idle_secs: Some(120),
        ..snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })
    };
    assert_eq!(chosen(&cold), Err(Full::Nowhere));
    // AND ONE SECOND UNDER IT STILL DOES, or the bound is one short and
    // nobody could tell from the outside.
    let warm = Snapshot {
        desk_idle_secs: Some(119),
        ..cold
    };
    assert_eq!(chosen(&warm), studio());
}

#[test]
fn a_locked_screen_disqualifies_the_desk_however_recent_its_last_keystroke() {
    // Locking necessarily postdates the last keystroke, so it is the
    // newest fact about the desk rather than an exception to the rule.
    let taken = Snapshot {
        desk_idle_secs: Some(0),
        screen_locked: Some(true),
        ..snapshot(in_room("2F - Kitchen"))
    };
    assert_eq!(chosen(&taken), Ok("2F - Kitchen".to_string()));
}

#[test]
fn a_desk_reading_nobody_could_take_never_competes() {
    // `None` must never coerce to zero, which would read as actively
    // typing and park every signal in the study for good.
    let taken = Snapshot {
        desk_idle_secs: None,
        ..snapshot(in_room("2F - Kitchen"))
    };
    assert_eq!(chosen(&taken), Ok("2F - Kitchen".to_string()));
}

#[test]
fn a_warm_desk_with_no_room_named_for_it_answers_nothing_rather_than_guessing() {
    let taken = Snapshot {
        desk_idle_secs: Some(0),
        desk_room: None,
        ..snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })
    };
    assert_eq!(chosen(&taken), Err(Full::NoDeskRoom));
}

#[test]
fn a_phone_off_the_home_network_answers_nothing_however_fresh_the_motion_is() {
    // Somebody is moving in the kitchen and it is not the operator. The
    // house is not theirs to narrow, so the whole routing stands.
    let taken = Snapshot {
        home: HomePresence::NotHome,
        ..snapshot(in_room("2F - Kitchen"))
    };
    assert_eq!(chosen(&taken), Err(Full::NotHome));
}

#[test]
fn a_router_that_could_not_answer_still_lets_motion_carry_the_lamps() {
    // Fresh motion in a watched room is itself evidence of a human in that
    // room, and it is better evidence than a router nobody could reach.
    // Read the other way, a machine with no router table would lose the
    // whole feature.
    let taken = Snapshot {
        home: HomePresence::Unknown,
        ..snapshot(in_room("2F - Kitchen"))
    };
    assert_eq!(chosen(&taken), Ok("2F - Kitchen".to_string()));
}

#[test]
fn a_desk_being_typed_at_outranks_a_router_that_says_nobody_is_home() {
    // The keyboard is the operator's own hand. A router that disagrees
    // with it is wrong about the router, not about the desk.
    let taken = Snapshot {
        desk_idle_secs: Some(0),
        home: HomePresence::NotHome,
        ..snapshot(in_room("2F - Kitchen"))
    };
    assert_eq!(chosen(&taken), studio());
}
