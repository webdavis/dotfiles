//! The bridge join's own tests: the shapes the two CLIP listings arrive in,
//! and the reading each one makes.
//!
//! ITS OWN FILE because the module's 500-line bound counts its tests, and the
//! join's prose earns its keep there. The SELECTION tests sit beside this in
//! `selection_tests`.

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
