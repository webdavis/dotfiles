//! The bridge side of the room sensor: two CLIP listings turned into one
//! reading for the state file.
//!
//! THE POLICY IS NOT HERE. `presence` decides what a reading means,
//! `presence_file` decides what a line looks like and `presence_instant`
//! decides what an instant is; this module decides only which watched room
//! moved last, which is the one question the bridge can answer. The four
//! change for four different reasons: a new backend, a new rule, a new
//! format, a new field shape.
//!
//! WHAT THE BRIDGE ACTUALLY SERVES, verified live on 2026-09-03 and the reason
//! the shapes below are refused the way they are: `grouped_motion` carries one
//! entry per room PLUS one owned by `bridge_home`, which is the whole house and
//! never a room; a room whose only sensor is switched off carries `motion: {}`
//! with no report inside it; and the `changed` instant carries MILLISECONDS
//! (`2026-09-03T17:20:09.413Z`). A motion body names no room, so the room name
//! is joined through the `room` listing by `owner.rid`, never by a name inside
//! the motion body: there is none.
//!
//! OPEN FACT, and the one thing here nobody can settle yet: the machine has
//! ZERO MotionAware areas, so whether an area's motion joins its room's
//! `grouped_motion` roll-up or arrives only as `convenience_area_motion` owned
//! by a `motion_area_configuration` is unverifiable. This reads the roll-up,
//! which is the shape that exists. See `docs/specs/daemon-jobs.md` for the one
//! GET that settles it once an area exists.

use crate::channels::hue::Bridge;
use crate::presence_file::{Edge, RawPresence};
use crate::presence_instant::instant_from_utc;

/// One poll: both listings, and the reading they make.
///
/// `None` IS A BRIDGE THAT DID NOT ANSWER, and the caller must publish nothing
/// for it. That is the whole fail-closed guarantee: a line that stops arriving
/// ages out to Unknown, where a line written anyway would pin the operator in
/// a room, or out of every room, on the word of a bridge that said nothing.
///
/// BOTH LISTINGS OR NOTHING, in `resolve_on_bridge`'s style: the motion body
/// carries rids and the room body carries the names they mean, so a poll
/// holding one of the two knows that something moved and not where.
pub fn poll<B: Bridge>(bridge: &B, watched: &[String], now: u64) -> Option<RawPresence> {
    let motion = bridge.get("grouped_motion")?;
    let rooms = bridge.get("room")?;
    reading(&motion, &rooms, watched, now)
}

/// What the two bodies say, as one reading. Pure, so the whole of the parse is
/// testable against bodies copied off the live bridge.
///
/// A BODY THIS CANNOT READ IS NOT AN ANSWER (`None`, so nothing is published),
/// and neither is a body carrying a WATCHED room whose report it cannot read,
/// while a body it CAN read holding no watched edge is the poll-only reading,
/// which says the bridge answered and no watched room has reported. Collapsing
/// those would let a garbled response claim the operator is nowhere.
pub fn reading(
    motion_json: &str,
    rooms_json: &str,
    watched: &[String],
    now: u64,
) -> Option<RawPresence> {
    let motion = data(motion_json)?;
    let rooms = data(rooms_json)?;
    // THE NEWEST EDGE AMONG THE WATCHED ROOMS, which is the only room this can
    // honestly name: an edge in a room nobody watches says nothing about where
    // the operator is, and letting one win would answer with a room the config
    // never listed.
    //
    // AND NO EDGE AT ALL WHEN ONE OF THEM IS UNREADABLE, which is why this is
    // a loop rather than a `filter_map`: the room whose report would not parse
    // may hold the newer edge, so the newest of the rest is a guess.
    //
    // COMPARED AT FULL PRECISION AND PUBLISHED IN WHOLE SECONDS. The bridge's
    // `changed` carries milliseconds, so reducing to the state file's format
    // BEFORE the comparison made two edges inside one second compare equal and
    // handed the answer to the order the bridge listed its rooms in.
    let mut newest: Option<((u64, u32), Edge)> = None;
    for entry in &motion {
        match edge_of(entry, &rooms, watched) {
            EntryEdge::Malformed => return None,
            EntryEdge::Found { at, edge } => {
                if newest.as_ref().is_none_or(|(held, _)| *held <= at) {
                    newest = Some((at, edge));
                }
            }
            EntryEdge::Irrelevant => {}
        }
    }
    Some(RawPresence {
        poll_epoch: now,
        edge: newest.map(|(_, edge)| edge),
    })
}

/// What one `grouped_motion` entry says about the watched rooms.
///
/// THREE ANSWERS RATHER THAN AN `Option`, because the third one changes what
/// gets published. An entry dropped for being unreadable used to leave the
/// poll-only line, and `classify` reads that as a FRESH "nowhere": a bridge
/// serving the same garbage every few seconds kept a false absence fresh for
/// as long as it kept serving it. Refusing the poll instead lets the last good
/// reading age out into `Stale`, which is the direction every other unknown in
/// this feature takes.
enum EntryEdge {
    /// A watched room's edge, with the full-precision instant it happened at
    /// beside the whole-second one the state file carries.
    Found { at: (u64, u32), edge: Edge },
    /// A watched room carrying a report this cannot read. The whole poll goes.
    Malformed,
    /// Everything that is neither: the house roll-up, a room nobody watches, a
    /// room the listing does not name, and a watched room whose sensors report
    /// nothing at all.
    Irrelevant,
}

/// The `.data[]` array of a CLIP response, or `None` for a body that has none.
///
/// ITS OWN COPY of `channels::hue`'s private helper, and deliberately not the
/// same function: that one answers an empty list for a body it could not read,
/// because a pulse with nothing to write is a no-op either way. Here the two
/// are different answers, and the difference is what a poll publishes.
fn data(clip_json: &str) -> Option<Vec<serde_json::Value>> {
    let body: serde_json::Value = serde_json::from_str(clip_json).ok()?;
    Some(body.get("data")?.as_array()?.clone())
}

/// One `grouped_motion` entry, as whichever of the three answers it is.
fn edge_of(
    entry: &serde_json::Value,
    rooms: &[serde_json::Value],
    watched: &[String],
) -> EntryEdge {
    let Some(room) = watched_room(entry, rooms, watched) else {
        return EntryEdge::Irrelevant;
    };
    // ABSENT FOR A ROOM WHOSE SENSORS ARE OFF, which serves `motion: {}`: no
    // report is no edge, never an edge at epoch zero, and never a refusal
    // either. It is the documented shape of a switched-off sensor, so the poll
    // it belongs to is a real answer with one room quiet in it.
    let Some(report) = entry.pointer("/motion/motion_report") else {
        return EntryEdge::Irrelevant;
    };
    // PAST THIS POINT A WATCHED ROOM SAID SOMETHING, so anything unreadable in
    // it is `Malformed` rather than silence.
    match report_edge(report, room) {
        Some(found) => found,
        None => EntryEdge::Malformed,
    }
}

/// The name of the watched room this entry belongs to, or `None` when it
/// belongs to none.
///
/// EVERY REFUSAL HERE IS IRRELEVANCE RATHER THAN MALFORMEDNESS, and it has to
/// be: until the entry is joined to a name, nothing knows whether it is even a
/// room the operator listed, and a bridge that serves the whole house would
/// otherwise let a garbled entry in a room nobody watches refuse every poll.
fn watched_room(
    entry: &serde_json::Value,
    rooms: &[serde_json::Value],
    watched: &[String],
) -> Option<String> {
    let owner = entry.get("owner")?;
    // THE HOUSE ROLL-UP IS NOT A ROOM. `bridge_home` reports every sensor in
    // the building, so its edge is the newest edge anywhere and it would win
    // every comparison above while naming nowhere in particular.
    if owner.get("rtype")?.as_str()? != "room" {
        return None;
    }
    let room = room_name(rooms, owner.get("rid")?.as_str()?)?;
    watched.contains(&room).then_some(room)
}

/// One `motion_report` as the edge it states, or `None` for a report this
/// cannot read.
fn report_edge(report: &serde_json::Value, room: String) -> Option<EntryEdge> {
    let at = instant_from_utc(report.get("changed")?.as_str()?)?;
    Some(EntryEdge::Found {
        at,
        edge: Edge {
            // THE FRACTION IS DROPPED HERE AND ONLY HERE, because the state
            // file carries whole seconds. `at` keeps it for the comparison.
            epoch: at.0,
            motion: report.get("motion")?.as_bool()?,
            room,
        },
    })
}

/// The name of the room with this id, or `None` when the listing does not hold
/// it. A room renamed or removed between two polls simply stops matching,
/// which is a room that no longer reports rather than an error.
fn room_name(rooms: &[serde_json::Value], rid: &str) -> Option<String> {
    rooms
        .iter()
        .find(|room| room.get("id").and_then(serde_json::Value::as_str) == Some(rid))?
        .pointer("/metadata/name")?
        .as_str()
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::{poll, reading};
    use crate::channels::hue::Bridge;
    use crate::presence_file::{Edge, RawPresence};

    /// The `grouped_motion` body, in the live shape: the house roll-up, a room
    /// whose sensor is off, and two rooms with edges.
    const MOTION: &str = r#"{"data":[
        {"owner":{"rid":"kitchen","rtype":"room"},"enabled":true,"motion":{}},
        {"owner":{"rid":"studio","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}},
        {"owner":{"rid":"hallway","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T16:27:05.600Z","motion":false}}},
        {"owner":{"rid":"door","rtype":"room"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:30:00.000Z","motion":true}}},
        {"owner":{"rid":"house","rtype":"bridge_home"},"enabled":true,
         "motion":{"motion_report":{"changed":"2026-09-03T17:59:59.000Z","motion":true}}}
    ]}"#;

    /// The `room` listing that names those ids.
    const ROOMS: &str = r#"{"data":[
        {"id":"kitchen","metadata":{"name":"2F - Kitchen"}},
        {"id":"studio","metadata":{"name":"3F - Studio"}},
        {"id":"hallway","metadata":{"name":"3F - Hallway"}},
        {"id":"door","metadata":{"name":"1F - Front door"}},
        {"id":"house","metadata":{"name":"the house"}}
    ]}"#;

    fn watched() -> Vec<String> {
        vec!["3F - Studio".to_string(), "2F - Kitchen".to_string()]
    }

    // --- reading ------------------------------------------------------------

    #[test]
    fn the_newest_edge_among_the_watched_rooms_is_the_one_reported() {
        // TWO WATCHED ROOMS THAT BOTH REPORTED, which is what makes this about
        // newest rather than about "the only one there was": the hallway's
        // edge is 53 minutes older than the studio's.
        let watched = vec!["3F - Studio".to_string(), "3F - Hallway".to_string()];
        assert_eq!(
            reading(MOTION, ROOMS, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: Some(Edge {
                    epoch: 1_788_456_009,
                    motion: false,
                    room: "3F - Studio".to_string(),
                }),
            })
        );
    }

    #[test]
    fn a_newer_edge_in_a_room_nobody_watches_never_displaces_a_watched_one() {
        // The front door is the newest ROOM edge in the body above, and it is
        // an arrival signal rather than a place the operator sits.
        let studio = reading(MOTION, ROOMS, &watched(), 1_788_456_100)
            .and_then(|raw| raw.edge)
            .expect("the studio edge");
        assert_eq!(studio.room, "3F - Studio");
    }

    #[test]
    fn the_house_roll_up_is_not_a_room() {
        // `bridge_home` holds the newest edge in the whole body, so a filter
        // that admitted it would win every comparison and name the house.
        let watched = vec!["the house".to_string()];
        assert_eq!(
            reading(MOTION, ROOMS, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_room_whose_sensor_is_switched_off_reports_no_edge() {
        // `motion: {}` is the live shape for it, and no report is no edge
        // rather than an edge at epoch zero.
        let watched = vec!["2F - Kitchen".to_string()];
        assert_eq!(
            reading(MOTION, ROOMS, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_watched_room_the_listing_does_not_name_reports_no_edge() {
        let watched = vec!["3F - Studio".to_string()];
        assert_eq!(
            reading(MOTION, r#"{"data":[]}"#, &watched, 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    #[test]
    fn a_body_this_cannot_read_is_no_poll_at_all_rather_than_a_nowhere() {
        // The difference is what gets published: no reading writes nothing and
        // ages out to Unknown, where a "nowhere" would be a claim.
        for (motion, rooms) in [
            ("not json", ROOMS),
            ("{}", ROOMS),
            (r#"{"data":{}}"#, ROOMS),
            (r#"{"errors":[{"description":"unauthorised"}]}"#, ROOMS),
            (MOTION, "not json"),
            (MOTION, r#"{"rooms":[]}"#),
        ] {
            assert_eq!(
                reading(motion, rooms, &watched(), 1_788_456_100),
                None,
                "{motion:.20} with {rooms:.20} answered anyway"
            );
        }
    }

    #[test]
    fn a_fraction_of_a_second_still_decides_which_watched_edge_is_newest() {
        // TWO EDGES INSIDE ONE SECOND, which is a shape the bridge really
        // serves: its `changed` carries milliseconds. Reducing both to the
        // same integer second before comparing them made 800ms apart compare
        // EQUAL, and the tie went to whichever room the bridge happened to
        // list last, so the answer turned on listing order rather than on
        // which room moved last. Driven in both orders for exactly that
        // reason.
        //
        // THE PUBLISHED EPOCH IS STILL WHOLE SECONDS: the fraction decides the
        // pick and the state file's format is unchanged.
        let newer = r#"{"owner":{"rid":"studio","rtype":"room"},
             "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.900Z","motion":true}}}"#;
        let older = r#"{"owner":{"rid":"hallway","rtype":"room"},
             "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.100Z","motion":false}}}"#;
        let watched = vec!["3F - Studio".to_string(), "3F - Hallway".to_string()];
        for (order, (first, second)) in [
            ("newest first", (newer, older)),
            ("newest last", (older, newer)),
        ] {
            assert_eq!(
                reading(
                    &format!("{{\"data\":[{first},{second}]}}"),
                    ROOMS,
                    &watched,
                    1_788_456_100
                )
                .and_then(|raw| raw.edge),
                Some(Edge {
                    epoch: 1_788_456_009,
                    motion: true,
                    room: "3F - Studio".to_string(),
                }),
                "listed {order}, the older edge was chosen"
            );
        }
    }

    #[test]
    fn a_report_a_watched_room_carries_but_this_cannot_read_is_no_poll_at_all() {
        // A REPORT THIS CANNOT READ IS NOT AN ABSENCE. Dropping the entry
        // published the poll-only line, which `classify` reads as a FRESH
        // "nowhere", so a bridge serving the same garbage every few seconds
        // kept a false absence fresh for as long as it kept serving it.
        // Refusing the poll lets the last good reading age out into Stale.
        let watched = vec!["3F - Studio".to_string()];
        for motion in [
            // A `changed` instant no shape here recognises.
            r#"{"data":[{"owner":{"rid":"studio","rtype":"room"},
                 "motion":{"motion_report":{"changed":"invalid","motion":true}}}]}"#,
            // `changed` absent from a report that exists.
            r#"{"data":[{"owner":{"rid":"studio","rtype":"room"},
                 "motion":{"motion_report":{"motion":true}}}]}"#,
            // `motion` absent, and `motion` that is not a boolean.
            r#"{"data":[{"owner":{"rid":"studio","rtype":"room"},
                 "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z"}}}]}"#,
            r#"{"data":[{"owner":{"rid":"studio","rtype":"room"},
                 "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z",
                 "motion":"yes"}}}]}"#,
        ] {
            assert_eq!(
                reading(motion, ROOMS, &watched, 1_788_456_100),
                None,
                "{motion:.70} published a reading anyway"
            );
        }
    }

    #[test]
    fn one_unreadable_watched_report_refuses_the_poll_beside_a_readable_one() {
        // THE OTHER HALF, and the reason a partial answer is not an answer:
        // the room this cannot read may hold the newer edge, so naming the
        // room it CAN read would be a guess dressed as a reading.
        let motion = r#"{"data":[
            {"owner":{"rid":"studio","rtype":"room"},
             "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}},
            {"owner":{"rid":"hallway","rtype":"room"},
             "motion":{"motion_report":{"changed":"invalid","motion":true}}}
        ]}"#;
        let watched = vec!["3F - Studio".to_string(), "3F - Hallway".to_string()];
        assert_eq!(reading(motion, ROOMS, &watched, 1_788_456_100), None);
    }

    #[test]
    fn an_unreadable_report_in_a_room_nobody_watches_costs_the_poll_nothing() {
        // ONLY A WATCHED ROOM CAN REFUSE A POLL. The bridge serves every room
        // in the house, so a garbled report in one the config never listed
        // would otherwise blind the sensor on rooms it does not even read.
        let motion = r#"{"data":[
            {"owner":{"rid":"studio","rtype":"room"},
             "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}},
            {"owner":{"rid":"door","rtype":"room"},
             "motion":{"motion_report":{"changed":"invalid","motion":true}}}
        ]}"#;
        assert_eq!(
            reading(motion, ROOMS, &watched(), 1_788_456_100)
                .and_then(|raw| raw.edge)
                .map(|edge| edge.room),
            Some("3F - Studio".to_string())
        );
    }

    #[test]
    fn a_body_with_an_empty_data_array_is_a_poll_that_saw_nothing() {
        assert_eq!(
            reading(r#"{"data":[]}"#, ROOMS, &watched(), 1_788_456_100),
            Some(RawPresence {
                poll_epoch: 1_788_456_100,
                edge: None,
            })
        );
    }

    // --- poll ---------------------------------------------------------------

    /// A bridge that answers each path from a table, and `None` for anything
    /// it was not given.
    struct ScriptedBridge(Vec<(&'static str, &'static str)>);

    impl Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(served, _)| *served == path)
                .map(|(_, body)| (*body).to_string())
        }

        fn put(&self, _path: &str, _body: &str) {
            unreachable!("the poll never writes to the bridge");
        }
    }

    #[test]
    fn a_poll_reads_both_listings_and_names_the_room() {
        let bridge = ScriptedBridge(vec![("grouped_motion", MOTION), ("room", ROOMS)]);
        assert_eq!(
            poll(&bridge, &watched(), 1_788_456_100)
                .and_then(|raw| raw.edge)
                .map(|edge| edge.room),
            Some("3F - Studio".to_string())
        );
    }

    #[test]
    fn a_bridge_that_answers_neither_listing_answers_no_reading() {
        for served in [
            vec![],
            vec![("grouped_motion", MOTION)],
            vec![("room", ROOMS)],
        ] {
            assert_eq!(
                poll(&ScriptedBridge(served), &watched(), 1_788_456_100),
                None
            );
        }
    }
}
