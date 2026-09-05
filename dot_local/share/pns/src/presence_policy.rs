//! What a room reading MEANS FOR THE LAMPS: the routing narrowed to the room
//! the operator is in, or left alone with a reason.
//!
//! ITS OWN MODULE, beside `presence` rather than inside it, for the split that
//! module's own doc draws: `presence` says what a READING means, and this says
//! what the lamps do about it.
//!
//! WHICH ROOM THE READINGS NAME IS `presence_room`'s QUESTION, not this one.
//! That module weighs the desk clock against the bridge's motion edge and
//! answers a room; this one takes that room to a lamp map. Its vocabulary,
//! `Snapshot` and `Full`, is re-exported here so a caller that only ever wants
//! the narrowing has one module to name.
//!
//! POLICY ONLY. Every function here is a total function of its arguments: no
//! bridge, no clock, no config file and no printing. The composition root
//! takes ONE snapshot of the world and hands it in.
//!
//! PRESENCE ONLY EVER NARROWS. Every way of not knowing leaves the routing
//! exactly as it was, and so does a narrowing that would leave no lamp at all:
//! silence is the one outcome this feature must never produce.

use crate::channels::hue::Routing;
use crate::presence_room::chosen;

pub use crate::presence_room::{Full, Snapshot};

/// What the narrowing did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Narrowing {
    /// Only the lamps the bridge places in this room were kept.
    To(String),
    /// The whole routing stands, and why.
    Full(Full),
}

/// The routing narrowed to the room the operator is in, and the decision that
/// says why.
pub fn narrow(mut routing: Routing, snapshot: &Snapshot) -> (Routing, Narrowing) {
    let room = match chosen(snapshot) {
        Ok(room) => room,
        Err(full) => return (routing, Narrowing::Full(full)),
    };
    // ASKED BEFORE ANYTHING IS DROPPED, so the whole routing is still here to
    // fall back to. Narrowing to a room the map routes nothing for would leave
    // the operator with no lamp at all and nothing said about why.
    if !routing.lamps.iter().any(|routed| holds(routed, &room)) {
        return (routing, Narrowing::Full(Full::NoLampIn(room)));
    }
    routing.lamps.retain(|routed| holds(routed, &room));
    (routing, Narrowing::To(room))
}

/// Whether one routed lamp belongs to a room.
///
/// THE BRIDGE'S OWN MEMBERSHIP, which `resolve` already joined off the room
/// listing and carried on every `Lamp`. A room derived from the lamp's NAME
/// instead would be a guess about a naming convention, and `resolve`'s own
/// rule is that the bridge's current membership is the truth: a lamp moved
/// between rooms answers its new room the moment the listing does.
fn holds(routed: &crate::channels::hue::Routed, room: &str) -> bool {
    routed.lamp.room.as_deref() == Some(room)
}

#[cfg(test)]
mod tests {
    use super::{Full, Narrowing, Snapshot, narrow};
    use crate::channels::hue::{Lamp, Routed, Routing};
    use crate::config::Behaviour;
    use crate::home::{DeviceKey, HomePresence};
    use crate::presence::{PresenceStatus, Unreadable};

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
}
