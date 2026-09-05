//! What a room reading MEANS FOR THE LAMPS: the routing narrowed to the room
//! the operator is in, or left alone with a reason.
//!
//! ITS OWN MODULE, beside `presence` rather than inside it, for the split that
//! module's own doc draws: `presence` says what a READING means, and this says
//! what the lamps do about it. The two change for different reasons, the
//! reading's when the poll or the file does and this one's when the routing
//! does.
//!
//! POLICY ONLY. Every function here is a total function of its arguments: no
//! bridge, no clock, no config file and no printing. The composition root
//! resolves the routing, takes the surface and the reading ONCE, calls this,
//! and appends the line it answers with.
//!
//! PRESENCE ONLY EVER NARROWS. Every way of not knowing leaves the routing
//! exactly as it was, and so does a narrowing that would leave no lamp at all:
//! silence is the one outcome this feature must never produce.

use crate::channels::hue::Routing;
use crate::presence::{PresenceStatus, Unreadable};
use crate::surface::Surface;

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
    /// The operator is away. The delivery plan already decides whether a lamp
    /// signals at all when nobody is here; narrowing it further says nothing.
    Away,
    /// The desk says they are at it, and no `desk_room` names which room that
    /// is.
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
pub fn narrow(
    mut routing: Routing,
    status: &PresenceStatus,
    surface: Surface,
    desk_room: Option<&str>,
) -> (Routing, Narrowing) {
    // THE SURFACE IS ASKED FIRST, and motion gets a vote only where it has
    // nothing to contradict. A desk being typed at is a stronger fact about
    // where the operator is than any sensor reading, and a cat crossing the
    // kitchen must not move the lamps off it.
    let room = match (surface, status) {
        (Surface::Desk, _) => match desk_room {
            Some(room) => room,
            None => return (routing, Narrowing::Full(Full::NoDeskRoom)),
        },
        // AWAY IS NOT A ROOM. The surface already says nobody is here to see
        // the lamp; shrinking what a returning operator can catch sight of is
        // the one thing a reading about an empty house could still cost.
        (Surface::Away, _) => return (routing, Narrowing::Full(Full::Away)),
        (_, PresenceStatus::Room { room, .. }) => room,
        (_, PresenceStatus::Nowhere { .. }) => return (routing, Narrowing::Full(Full::Nowhere)),
        (_, PresenceStatus::Unknown(reason)) => {
            return (routing, Narrowing::Full(Full::Unknown(*reason)));
        }
    };
    let room = room.to_string();
    // ASKED BEFORE ANYTHING IS DROPPED, so the whole routing is still here to
    // fall back to. Narrowing to a room the map routes nothing for would leave
    // the operator with no lamp at all and nothing said about why.
    if !routing.lamps.iter().any(|routed| holds(routed, &room)) {
        return (routing, Narrowing::Full(Full::NoLampIn(room)));
    }
    routing.lamps.retain(|routed| holds(routed, &room));
    (routing, Narrowing::To(room))
}

/// The narrowing as one phrase, for the journal and for the doctor.
///
/// THE ROOM IS DEBUG-QUOTED, and that is a guard rather than a style: the name
/// is the bridge's own text, this phrase is appended to a ring file one line
/// at a time and printed to a terminal by `pns doctor`, and `{:?}` escapes the
/// newline that would otherwise forge a second entry.
pub fn said(narrowing: &Narrowing) -> String {
    match narrowing {
        Narrowing::To(room) => format!("{room:?}"),
        Narrowing::Full(full) => format!("nothing ({})", why(full)),
    }
}

/// Why a routing was left whole, in one phrase.
fn why(full: &Full) -> String {
    match full {
        Full::Away => "away".to_string(),
        Full::NoDeskRoom => "at the desk, and no desk_room says which room that is".to_string(),
        Full::Nowhere => "motion in no watched room".to_string(),
        Full::Unknown(reason) => format!("unknown: {}", crate::presence::unreadable_said(reason)),
        Full::NoLampIn(room) => format!("no lamp in {room:?}"),
    }
}

/// One narrowing decision as one journal line.
///
/// `narrowed=` IS LAST and everything after it is the phrase, which is the
/// whole parse its one reader does. Nothing before it can be mistaken for the
/// marker: the only free text on the line is a room name, and `said` and the
/// reading below both quote theirs.
pub fn journal_line(
    now: Option<u64>,
    status: &PresenceStatus,
    surface: Surface,
    narrowing: &Narrowing,
) -> String {
    format!(
        "{epoch} presence={reading} surface={surface:?} narrowed={narrowed}",
        epoch = now.map_or_else(|| "-".to_string(), |now| now.to_string()),
        reading = reading_said(status),
        narrowed = said(narrowing),
    )
}

/// What the reading itself says, quoted the way `said` quotes its own room and
/// for the same reason.
fn reading_said(status: &PresenceStatus) -> String {
    match status {
        PresenceStatus::Room { room, age_secs } => format!("room {room:?} ({age_secs}s ago)"),
        PresenceStatus::Nowhere { poll_age_secs } => format!("nowhere (poll {poll_age_secs}s ago)"),
        PresenceStatus::Unknown(reason) => {
            format!("unknown ({})", crate::presence::unreadable_said(reason))
        }
    }
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
    use super::{Full, Narrowing, journal_line, narrow};
    use crate::channels::hue::{Lamp, Routed, Routing};
    use crate::config::Behaviour;
    use crate::presence::{PresenceStatus, Unreadable};
    use crate::surface::Surface;

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

    /// The lamps the operator's own values file reaches: three room blocks
    /// (Studio, MBedroom, Kitchen) and four lamp blocks, resolved against the
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

    /// A fresh reading naming one room.
    fn in_room(room: &str) -> PresenceStatus {
        PresenceStatus::Room {
            room: room.to_string(),
            age_secs: 0,
        }
    }

    #[test]
    fn a_fresh_room_reading_keeps_exactly_the_lamps_that_room_holds() {
        let (narrowed, decision) = narrow(
            operators_routing(),
            &in_room("3F - MBedroom"),
            Surface::Mobile,
            Some("3F - Studio"),
        );
        assert_eq!(
            names(&narrowed),
            ["3F - MBedroom - HCL1", "3F - MBedroom - HCL3"]
        );
        assert_eq!(decision, Narrowing::To("3F - MBedroom".to_string()));
    }

    #[test]
    fn a_fresh_poll_that_found_no_watched_room_narrows_nothing_and_journals_why() {
        // The operator may be standing in a room with no sensor at all; not
        // knowing costs the narrowing and nothing else.
        let (narrowed, decision) = narrow(
            operators_routing(),
            &PresenceStatus::Nowhere { poll_age_secs: 3 },
            Surface::Mobile,
            Some("3F - Studio"),
        );
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(decision, Narrowing::Full(Full::Nowhere));
        assert!(
            journal_line(
                Some(1_700_000_000),
                &PresenceStatus::Nowhere { poll_age_secs: 3 },
                Surface::Mobile,
                &decision,
            )
            .ends_with("narrowed=nothing (motion in no watched room)"),
            "the line says what was not narrowed and why"
        );
    }

    #[test]
    fn every_unreadable_reading_narrows_nothing_and_journals_its_own_reason() {
        // FIVE DIFFERENT THINGS TO GO AND FIX, so the line names which one:
        // collapsed to a single "unknown" the operator cannot tell a daemon
        // that stopped from a room this config never watched.
        for (reason, named) in [
            (Unreadable::NoReading, "no reading"),
            (Unreadable::NoClock, "the clock could not be read"),
            (
                Unreadable::Stale { poll_age_secs: 90 },
                "stale, poll 90s old",
            ),
            (Unreadable::Future, "future epoch"),
            (
                Unreadable::NotWatched,
                "the reported room is not one this config watches",
            ),
        ] {
            let status = PresenceStatus::Unknown(reason);
            let (narrowed, decision) = narrow(
                operators_routing(),
                &status,
                Surface::Mobile,
                Some("3F - Studio"),
            );
            assert_eq!(names(&narrowed), names(&operators_routing()), "{reason:?}");
            assert_eq!(
                decision,
                Narrowing::Full(Full::Unknown(reason)),
                "{reason:?}"
            );
            assert!(
                journal_line(Some(1_700_000_000), &status, Surface::Mobile, &decision)
                    .ends_with(&format!("narrowed=nothing (unknown: {named})")),
                "{reason:?} names its own reason"
            );
        }
    }

    #[test]
    fn a_desk_that_says_they_are_at_it_beats_fresh_motion_in_another_room() {
        // Motion gets a vote only once the desk has gone idle: a cat crossing
        // the kitchen must not move the lamps off a desk being typed at.
        let (narrowed, decision) = narrow(
            operators_routing(),
            &in_room("2F - Kitchen"),
            Surface::Desk,
            Some("3F - Studio"),
        );
        assert_eq!(
            names(&narrowed),
            ["3F - Studio - HCL1", "3F - Studio - HCL3"]
        );
        assert_eq!(decision, Narrowing::To("3F - Studio".to_string()));
    }

    #[test]
    fn an_away_surface_narrows_nothing_however_fresh_the_motion_is() {
        // Nobody is here to see the lamp. Whether it signals at all is the
        // delivery plan's question, and narrowing it to a room would only
        // shrink what a returning operator can catch sight of.
        let (narrowed, decision) = narrow(
            operators_routing(),
            &in_room("2F - Kitchen"),
            Surface::Away,
            Some("3F - Studio"),
        );
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(decision, Narrowing::Full(Full::Away));
    }

    #[test]
    fn a_room_holding_no_routed_lamp_falls_back_to_the_whole_routing() {
        // SILENCE IS THE ONE OUTCOME THIS FEATURE MUST NEVER PRODUCE. Standing
        // in a room the map routes nothing for is not a reason to stop
        // signalling; it is a reason to signal everywhere, and to say so.
        let (narrowed, decision) = narrow(
            operators_routing(),
            &in_room("3F - Hallway"),
            Surface::Mobile,
            Some("3F - Studio"),
        );
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(
            decision,
            Narrowing::Full(Full::NoLampIn("3F - Hallway".to_string()))
        );
        assert!(
            journal_line(
                Some(1_700_000_000),
                &in_room("3F - Hallway"),
                Surface::Mobile,
                &decision,
            )
            .ends_with(r#"narrowed=nothing (no lamp in "3F - Hallway")"#),
            "the line names the room that held nothing"
        );
    }

    #[test]
    fn a_desk_with_no_room_named_for_it_narrows_nothing_rather_than_guessing() {
        // The operator switched presence on and never said where the desk is.
        // Reaching for the motion vote instead would move the lamps off a desk
        // being typed at, which is the one thing the desk rule exists to stop.
        let (narrowed, decision) = narrow(
            operators_routing(),
            &in_room("2F - Kitchen"),
            Surface::Desk,
            None,
        );
        assert_eq!(names(&narrowed), names(&operators_routing()));
        assert_eq!(decision, Narrowing::Full(Full::NoDeskRoom));
    }

    #[test]
    fn the_journal_line_carries_the_reading_the_surface_and_the_room_chosen() {
        // All three, because the answer is only explicable from all three: the
        // same room can be chosen off motion or off the desk, and the same
        // motion can be overruled or obeyed.
        assert_eq!(
            journal_line(
                Some(1_700_000_000),
                &in_room("3F - MBedroom"),
                Surface::Mobile,
                &Narrowing::To("3F - MBedroom".to_string()),
            ),
            r#"1700000000 presence=room "3F - MBedroom" (0s ago) surface=Mobile narrowed="3F - MBedroom""#
        );
    }

    #[test]
    fn a_room_name_carrying_a_newline_cannot_forge_a_second_journal_entry() {
        // The name is the bridge's own text and the line is appended to a ring
        // one line at a time, so the quoting is a guard rather than a style.
        let line = journal_line(
            None,
            &in_room(
                "3F - Studio
1700000000 presence=room",
            ),
            Surface::Mobile,
            &Narrowing::To(
                "3F - Studio
forged"
                    .to_string(),
            ),
        );
        assert!(!line.contains('\n'), "{line}");
    }
}
