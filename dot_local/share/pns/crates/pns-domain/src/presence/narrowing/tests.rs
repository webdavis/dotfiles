use super::{Full, Narrowing, Snapshot, narrow};
use crate::home::identity::DeviceKey;
use crate::home::reading::HomePresence;
use crate::lamps::config::Behaviour;
use crate::lamps::inventory::Lamp;
use crate::lamps::resolve::{Routed, Routing};
use crate::presence::status::{PresenceStatus, Unreadable};

/// One lamp as the bridge places it, routed for one behaviour.
fn routed(name: &str, room: &str) -> Routed {
    Routed {
        lamp: Lamp {
            id: name.to_string(),
            name: name.to_string(),
            room: Some(room.to_string()),
            zones: Vec::new(),
        },
        shows: vec![Behaviour::Done],
        dim: None,
    }
}

/// The lamps the operator's own values file reaches, resolved against the
/// bridge's real membership (verified live 2026-09-04).
fn operators_routing() -> Routing {
    Routing {
        lamps: vec![
            routed("1F - Front door - HCL1", "1F - Front door"),
            routed("2F - Kitchen - HCD3", "2F - Kitchen"),
            routed("2F - Kitchen - HCD6", "2F - Kitchen"),
            routed("3F - MBedroom - HCL1", "3F - MBedroom"),
            routed("3F - MBedroom - HCL3", "3F - MBedroom"),
            routed("3F - Studio - HCL1", "3F - Studio"),
            routed("3F - Studio - HCL3", "3F - Studio"),
        ],
        ..Routing::default()
    }
}

/// The lamp names one routing carries, in order.
fn names(routing: &Routing) -> Vec<&str> {
    routing
        .lamps
        .iter()
        .map(|routed| routed.lamp.name.as_str())
        .collect()
}

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

#[test]
fn a_fresh_room_reading_keeps_exactly_the_lamps_that_room_holds() {
    let (narrowed, decision) = narrow(operators_routing(), &snapshot(in_room("3F - MBedroom")));
    assert_eq!(
        names(&narrowed),
        ["3F - MBedroom - HCL1", "3F - MBedroom - HCL3"]
    );
    assert_eq!(decision, Narrowing::To("3F - MBedroom".to_string()));
}

#[test]
fn a_room_the_desk_names_reaches_the_lamps_the_same_way_a_reading_does() {
    // THE DESK'S OWN HALF OF THE WIRING, and the operator's commonest
    // case: typing at the keyboard while a cat crosses the kitchen. Which
    // room the two sensors name is `presence_room`'s question; that the
    // answer reaches the lamp map at all is this one's.
    let taken = Snapshot {
        desk_idle_secs: Some(0),
        ..snapshot(in_room("2F - Kitchen"))
    };
    let (narrowed, decision) = narrow(operators_routing(), &taken);
    assert_eq!(
        names(&narrowed),
        ["3F - Studio - HCL1", "3F - Studio - HCL3"]
    );
    assert_eq!(decision, Narrowing::To("3F - Studio".to_string()));
}

#[test]
fn a_fresh_poll_that_found_no_watched_room_narrows_nothing_and_records_why() {
    // The operator may be standing in a room with no sensor at all; not
    // knowing costs the narrowing and nothing else.
    let taken = snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 });
    let (narrowed, decision) = narrow(operators_routing(), &taken);
    assert_eq!(names(&narrowed), names(&operators_routing()));
    assert_eq!(decision, Narrowing::Full(Full::Nowhere));
}

#[test]
fn a_reading_no_room_could_be_read_out_of_narrows_nothing_and_keeps_its_reason() {
    // ONE ARM PER WAY OF NOT KNOWING, because each is a different thing to
    // go and fix; the five wordings are pinned in `presence_journal`.
    let taken = snapshot(PresenceStatus::Unknown(Unreadable::Stale {
        poll_age_secs: 90,
    }));
    let (narrowed, decision) = narrow(operators_routing(), &taken);
    assert_eq!(names(&narrowed), names(&operators_routing()));
    assert_eq!(
        decision,
        Narrowing::Full(Full::Unknown(Unreadable::Stale { poll_age_secs: 90 }))
    );
}

#[test]
fn a_desk_and_a_motion_edge_that_disagree_leave_every_lamp_alone() {
    // AMBIGUITY IS NOT A ROOM. Somebody is in the kitchen and the operator
    // may still be at the keyboard; narrowed to either one, the other
    // half of the house goes dark for a body that is standing in it.
    let taken = Snapshot {
        desk_idle_secs: Some(3),
        ..snapshot(in_room("2F - Kitchen"))
    };
    let (narrowed, decision) = narrow(operators_routing(), &taken);
    assert_eq!(names(&narrowed), names(&operators_routing()));
    assert_eq!(
        decision,
        Narrowing::Full(Full::Ambiguous {
            desk: "3F - Studio".to_string(),
            motion: "2F - Kitchen".to_string(),
        })
    );
}

#[test]
fn a_room_holding_no_routed_lamp_falls_back_to_the_whole_routing() {
    // SILENCE IS THE ONE OUTCOME THIS FEATURE MUST NEVER PRODUCE. Standing
    // in a room the map routes nothing for is not a reason to stop
    // signalling; it is a reason to signal everywhere, and to say so.
    let taken = snapshot(in_room("3F - Hallway"));
    let (narrowed, decision) = narrow(operators_routing(), &taken);
    assert_eq!(names(&narrowed), names(&operators_routing()));
    assert_eq!(
        decision,
        Narrowing::Full(Full::NoLampIn("3F - Hallway".to_string()))
    );
}
