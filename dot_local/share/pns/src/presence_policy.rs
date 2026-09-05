//! What a room reading MEANS FOR THE LAMPS: the routing narrowed to the room
//! the operator is in, or left alone with a reason.
//!
//! ITS OWN MODULE, beside `presence` rather than inside it, for the split that
//! module's own doc draws: `presence` says what a READING means, and this says
//! what the lamps do about it.
//!
//! IT DOES NOT READ THE NOTIFICATION SURFACE, and that is the correction this
//! module exists to hold. `surface::Surface` answers WHERE THE OPERATOR'S EYES
//! ARE, for picking a notifier; it is `Desk` for two minutes after the last
//! keystroke and `Away` whenever neither the keyboard nor the phone has been
//! touched lately. Neither is a claim about which room a body is standing in:
//! read as location, `Desk` ignores fresh motion in the kitchen for two
//! minutes after the operator walks out of the study, and `Away` reads a phone
//! sitting in a pocket at home as an empty house. The inputs here are physical
//! instead: the desk's own idle clock, the bridge's motion edge, and the
//! router's answer about whether the phone is on the home network.
//!
//! POLICY ONLY. Every function here is a total function of its arguments: no
//! bridge, no clock, no config file and no printing. The composition root
//! takes ONE snapshot of the world and hands it in.
//!
//! PRESENCE ONLY EVER NARROWS. Every way of not knowing leaves the routing
//! exactly as it was, and so does a narrowing that would leave no lamp at all:
//! silence is the one outcome this feature must never produce.

use crate::channels::hue::Routing;
use crate::home::HomePresence;
use crate::presence::{PresenceStatus, Unreadable};

/// Everything the narrowing is a function of, taken at ONE moment.
///
/// ONE STRUCT AND NOT SIX ARGUMENTS, for the reason `engine`'s own
/// `SurfaceReading` is one: these are one judgement over one set of readings,
/// and a caller free to take any of them again further down is a caller free
/// to take it at a different moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// What the bridge's motion roll-up says, already aged and judged.
    pub status: PresenceStatus,
    /// Seconds since the desk keyboard was touched. `None` is a reading
    /// nobody could take, which is never the same as zero.
    pub desk_idle_secs: Option<u64>,
    /// The desk display's lock. ONLY `Some(true)` DISQUALIFIES THE DESK, which
    /// is `surface::surface`'s own rule and is newest-signal-wins rather than
    /// an exception to it: locking necessarily postdates the last keystroke.
    pub screen_locked: Option<bool>,
    /// Whether the phone is on the home network.
    pub home: HomePresence,
    /// The room the desk is in, when the operator named one.
    pub desk_room: Option<String>,
    /// How long a desk reading still speaks for where the operator IS.
    pub desk_stale_after_secs: u64,
    /// The clock every age above was aged against, carried so the record is
    /// stamped with the moment the readings were taken.
    pub now: Option<u64>,
}

/// What the narrowing did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Narrowing {
    /// Only the lamps the bridge places in this room were kept.
    To(String),
    /// The whole routing stands, and why.
    Full(Full),
}

/// Why a routing was left whole. EVERY VARIANT IS A DIFFERENT THING TO GO AND
/// FIX, which is what an `Option` here could not have carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Full {
    /// The router says the phone is not on the home network, so the motion is
    /// somebody or something else.
    NotHome,
    /// The desk is the freshest thing there is and no `desk_room` says which
    /// room it is in.
    NoDeskRoom,
    /// A fresh poll found motion in no watched room. NOT a claim that nobody
    /// is home: the room they are in may have no sensor.
    Nowhere,
    /// No usable reading, and which kind.
    Unknown(Unreadable),
    /// The room holds no routed lamp, so narrowing to it would light nothing.
    NoLampIn(String),
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

/// Which room the readings put the operator in, or why none of them does.
///
/// NEWEST SIGNAL WINS, which is `surface::surface`'s own rule applied to two
/// clocks of the same kind. The desk reports how long since a hand touched it
/// and the bridge reports how long since a body moved in a room; both answer
/// "how long since a human did something HERE", so the fresher one says where
/// that human is. THE TIE GOES TO THE DESK, where a hand is what made the
/// reading: nobody is in two rooms at once, and a cat crossing the kitchen
/// must not move the lamps off a keyboard being typed on.
fn chosen(snapshot: &Snapshot) -> Result<String, Full> {
    let desk_age = desk_age(snapshot);
    let motion = match &snapshot.status {
        PresenceStatus::Room { room, age_secs } => Ok((room, *age_secs)),
        PresenceStatus::Nowhere { .. } => Err(Full::Nowhere),
        PresenceStatus::Unknown(reason) => Err(Full::Unknown(*reason)),
    };
    let desk_wins = match (desk_age, &motion) {
        (Some(desk), Ok((_, motion))) => desk <= *motion,
        (Some(_), Err(_)) => true,
        (None, _) => false,
    };
    if desk_wins {
        if let Some(room) = &snapshot.desk_room {
            return Ok(room.clone());
        }
        // A desk that would have won with no room named for it is a different
        // thing to go and fix from a poll that found nobody, and it is only
        // worth saying when nothing else could have answered either.
        if motion.is_err() {
            return Err(Full::NoDeskRoom);
        }
    }
    let (room, _) = motion?;
    // THE ROUTER IS ASKED ABOUT MOTION AND NEVER ABOUT THE DESK. A keyboard
    // being typed on is the operator's own hand, so a router that says nobody
    // is home while the desk is warm is wrong about the router. Motion has no
    // such author: it is a body, and the router is what says whose.
    //
    // ONLY `NotHome` GATES. `Unknown` is a router nobody could reach, or a
    // machine that never armed one, and read as absence it would take the
    // whole feature away; fresh motion in a watched room is itself evidence of
    // a human in that room.
    if snapshot.home == HomePresence::NotHome {
        return Err(Full::NotHome);
    }
    Ok(room.clone())
}

/// How long since the desk was touched, when that reading still speaks for
/// where the operator IS.
///
/// THREE WAYS IT SPEAKS FOR NOTHING, and none of them may become a zero:
/// unreadable (never the same as "actively typing"), a locked screen (which
/// necessarily postdates the last keystroke, so it is the NEWEST fact about
/// the desk), and older than the bound the operator set.
fn desk_age(snapshot: &Snapshot) -> Option<u64> {
    snapshot
        .desk_idle_secs
        .filter(|_| snapshot.screen_locked != Some(true))
        .filter(|age| *age < snapshot.desk_stale_after_secs)
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

    /// The phone answering on the home network.
    fn at_home() -> HomePresence {
        HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }
    }

    /// The operator's own snapshot, with the desk cold and the phone home.
    fn snapshot(status: PresenceStatus) -> Snapshot {
        Snapshot {
            status,
            desk_idle_secs: None,
            screen_locked: Some(false),
            home: at_home(),
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
    fn a_fresh_poll_that_found_no_watched_room_narrows_nothing_and_records_why() {
        // The operator may be standing in a room with no sensor at all; not
        // knowing costs the narrowing and nothing else.
        let taken = snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 });
        let (narrowed, decision) = narrow(operators_routing(), &taken);
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(decision, Narrowing::Full(Full::Nowhere));
    }

    #[test]
    fn every_unreadable_reading_narrows_nothing_and_keeps_its_own_reason() {
        // FIVE DIFFERENT THINGS TO GO AND FIX, and the variant is what carries
        // which: collapsed to a single "unknown" the operator cannot tell a
        // daemon that stopped from a room this config never watched. The five
        // WORDINGS are pinned where they are written down, in
        // `presence_journal::every_way_a_routing_can_be_left_whole_names_its_own_reason`.
        for reason in [
            Unreadable::NoReading,
            Unreadable::NoClock,
            Unreadable::Stale { poll_age_secs: 90 },
            Unreadable::Future,
            Unreadable::NotWatched,
        ] {
            let taken = snapshot(PresenceStatus::Unknown(reason));
            let (narrowed, decision) = narrow(operators_routing(), &taken);
            assert_eq!(names(&narrowed), names(&operators_routing()), "{reason:?}");
            assert_eq!(
                decision,
                Narrowing::Full(Full::Unknown(reason)),
                "{reason:?}"
            );
        }
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

    // --- where the operator IS, rather than where their attention is --------

    #[test]
    fn a_desk_still_being_typed_at_beats_motion_of_the_same_age() {
        // A cat crossing the kitchen must not move the lamps off a keyboard
        // somebody is typing on: nobody is in two rooms at once, and only one
        // of these two readings is made by a human hand.
        let (narrowed, decision) = narrow(
            operators_routing(),
            &Snapshot {
                desk_idle_secs: Some(0),
                ..snapshot(in_room("2F - Kitchen"))
            },
        );
        assert_eq!(
            names(&narrowed),
            ["3F - Studio - HCL1", "3F - Studio - HCL3"]
        );
        assert_eq!(decision, Narrowing::To("3F - Studio".to_string()));
    }

    #[test]
    fn walking_out_of_the_study_hands_the_lamps_over_as_soon_as_the_desk_is_older() {
        // THE LEAVE-THE-DESK TIMELINE. The desk clock and the motion edge
        // measure the same thing, how long since a human did something here,
        // so the fresher one says where they are. Read off the notification
        // surface instead, the desk held every lamp for a flat two minutes
        // while the operator stood in the kitchen.
        for desk_idle_secs in [60, 119, 121, 300] {
            let (narrowed, decision) = narrow(
                operators_routing(),
                &Snapshot {
                    desk_idle_secs: Some(desk_idle_secs),
                    ..snapshot(in_room("2F - Kitchen"))
                },
            );
            assert_eq!(
                names(&narrowed),
                ["2F - Kitchen - HCD3", "2F - Kitchen - HCD6"],
                "at {desk_idle_secs}s idle the fresh kitchen edge is the newer signal"
            );
            assert_eq!(decision, Narrowing::To("2F - Kitchen".to_string()));
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
        assert_eq!(
            narrow(operators_routing(), &cold).1,
            Narrowing::Full(Full::Nowhere)
        );
        // AND ONE SECOND UNDER IT STILL DOES, or the bound is one short and
        // nobody could tell from the outside.
        let warm = Snapshot {
            desk_idle_secs: Some(119),
            ..cold
        };
        assert_eq!(
            narrow(operators_routing(), &warm).1,
            Narrowing::To("3F - Studio".to_string())
        );
    }

    #[test]
    fn a_locked_screen_disqualifies_the_desk_however_recent_its_last_keystroke() {
        // Locking necessarily postdates the last keystroke, so it is the
        // newest fact about the desk rather than an exception to the rule.
        let (_, decision) = narrow(
            operators_routing(),
            &Snapshot {
                desk_idle_secs: Some(0),
                screen_locked: Some(true),
                ..snapshot(in_room("2F - Kitchen"))
            },
        );
        assert_eq!(decision, Narrowing::To("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_desk_reading_nobody_could_take_never_competes() {
        // `None` must never coerce to zero, which would read as actively
        // typing and park every signal in the study for good.
        let (_, decision) = narrow(
            operators_routing(),
            &Snapshot {
                desk_idle_secs: None,
                ..snapshot(in_room("2F - Kitchen"))
            },
        );
        assert_eq!(decision, Narrowing::To("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_warm_desk_with_no_room_named_for_it_narrows_nothing_rather_than_guessing() {
        let taken = Snapshot {
            desk_idle_secs: Some(0),
            desk_room: None,
            ..snapshot(PresenceStatus::Nowhere { poll_age_secs: 3 })
        };
        let (narrowed, decision) = narrow(operators_routing(), &taken);
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(decision, Narrowing::Full(Full::NoDeskRoom));
    }

    #[test]
    fn a_phone_at_home_lets_fresh_motion_carry_the_lamps() {
        let (narrowed, decision) = narrow(operators_routing(), &snapshot(in_room("2F - Kitchen")));
        assert_eq!(
            names(&narrowed),
            ["2F - Kitchen - HCD3", "2F - Kitchen - HCD6"]
        );
        assert_eq!(decision, Narrowing::To("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_phone_off_the_home_network_narrows_nothing_however_fresh_the_motion_is() {
        // Somebody is moving in the kitchen and it is not the operator. The
        // house is not theirs to narrow, so the whole routing stands.
        let taken = Snapshot {
            home: HomePresence::NotHome,
            ..snapshot(in_room("2F - Kitchen"))
        };
        let (narrowed, decision) = narrow(operators_routing(), &taken);
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(decision, Narrowing::Full(Full::NotHome));
    }

    #[test]
    fn a_router_that_could_not_answer_still_lets_motion_carry_the_lamps() {
        // Fresh motion in a watched room is itself evidence of a human in that
        // room, and it is better evidence than a router nobody could reach.
        // Read the other way, a machine with no router table would lose the
        // whole feature.
        let (_, decision) = narrow(
            operators_routing(),
            &Snapshot {
                home: HomePresence::Unknown,
                ..snapshot(in_room("2F - Kitchen"))
            },
        );
        assert_eq!(decision, Narrowing::To("2F - Kitchen".to_string()));
    }

    #[test]
    fn a_desk_being_typed_at_outranks_a_router_that_says_nobody_is_home() {
        // The keyboard is the operator's own hand. A router that disagrees
        // with it is wrong about the router, not about the desk.
        let (_, decision) = narrow(
            operators_routing(),
            &Snapshot {
                desk_idle_secs: Some(0),
                home: HomePresence::NotHome,
                ..snapshot(in_room("2F - Kitchen"))
            },
        );
        assert_eq!(decision, Narrowing::To("3F - Studio".to_string()));
    }
}
