//! Which watched entry the reading is taken from, tested apart from the
//! parse's own shapes.
//!
//! ITS OWN FILE because the module's is full: the standing bound on a Rust
//! file is 500 lines including its tests, and `presence_hue.rs` is at 482.
//! What sits here is the SELECTION: the entries that are not evidence at all,
//! and the order two entries that are evidence get compared in.

use super::reading;
use crate::presence_file::{Edge, RawPresence};

/// The two watched rooms these tests choose between.
const ROOMS: &str = r#"{"data":[
    {"id":"studio","metadata":{"name":"3F - Studio"}},
    {"id":"hallway","metadata":{"name":"3F - Hallway"}}
]}"#;

fn watched() -> Vec<String> {
    vec!["3F - Studio".to_string(), "3F - Hallway".to_string()]
}

/// The sibling that is real evidence in every test below: older than the
/// studio entry beside it, so it is chosen only when the studio's is not
/// evidence at all.
const HALLWAY: &str = r#"{"owner":{"rid":"hallway","rtype":"room"},
     "motion":{"motion_valid":true,
      "motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}}"#;

/// That sibling's edge, as the reading a whole poll answers with.
fn hallway_reading() -> Option<RawPresence> {
    Some(RawPresence {
        poll_epoch: 1_788_456_100,
        edge: Some(Edge {
            epoch: 1_788_456_009,
            motion: false,
            room: "3F - Hallway".to_string(),
        }),
    })
}

#[test]
fn a_room_the_bridge_says_has_no_valid_motion_is_not_evidence_of_anything() {
    // `motion_valid: false` IS THE BRIDGE DISOWNING THE READING BESIDE IT, and
    // both shapes of it were read as data. A complete report under it became a
    // Found edge, and the newest one wins, so a room the bridge said nothing
    // valid about could name where the operator is. A partial report under it
    // was Malformed, which refuses the WHOLE poll and throws away the sibling
    // rooms that did report: an invalid sensor in one room blinded the sensor
    // in every other.
    //
    // NEITHER IS A REFUSAL AND NEITHER IS AN EDGE. It is the same answer a
    // switched-off sensor gets: this room said nothing, the others still
    // count. The studio entry is NEWER than the hallway's in both, so a
    // regression that reads either as data names the studio.
    for (shape, studio) in [
        (
            "a complete report the bridge disowned",
            r#"{"owner":{"rid":"studio","rtype":"room"},
             "motion":{"motion_valid":false,
              "motion_report":{"changed":"2026-09-03T17:30:00.000Z","motion":true}}}"#,
        ),
        (
            "a partial report the bridge disowned",
            r#"{"owner":{"rid":"studio","rtype":"room"},
             "motion":{"motion_valid":false,
              "motion_report":{"changed":"2026-09-03T17:30:00.000Z"}}}"#,
        ),
    ] {
        assert_eq!(
            reading(
                &format!("{{\"data\":[{studio},{HALLWAY}]}}"),
                ROOMS,
                &watched(),
                1_788_456_100
            ),
            hallway_reading(),
            "{shape}: the valid sibling room was not the reading"
        );
    }
}

#[test]
fn a_partial_report_the_bridge_never_disowned_still_refuses_the_poll() {
    // THE OTHER HALF, and the line the fix above must not cross: a watched
    // room whose report is unreadable for no stated reason may hold the newer
    // edge, so naming the room this CAN read would be a guess. Only the
    // bridge's own `motion_valid: false` turns that refusal into silence.
    let studio = r#"{"owner":{"rid":"studio","rtype":"room"},
         "motion":{"motion_report":{"changed":"2026-09-03T17:30:00.000Z"}}}"#;
    assert_eq!(
        reading(
            &format!("{{\"data\":[{studio},{HALLWAY}]}}"),
            ROOMS,
            &watched(),
            1_788_456_100
        ),
        None
    );
}

#[test]
fn two_edges_at_the_very_same_instant_pick_the_same_room_in_either_order() {
    // THE LISTING ORDER IS NOT STABLE, so a comparison that lets the later
    // entry win a tie answers a different room from one poll to the next with
    // nothing on the bridge having changed. The instants here are equal to the
    // millisecond, which the bridge really can serve: `changed` is the
    // instant the SENSOR reported, and two sensors reporting inside the same
    // millisecond is a coincidence rather than an impossibility.
    //
    // THE ROOM NAME IS THE TIE-BREAKER, so the answer is a fact about the
    // reading rather than about the response: `3F - Studio` sorts after
    // `3F - Hallway`.
    let studio = r#"{"owner":{"rid":"studio","rtype":"room"},
         "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":true}}}"#;
    let hallway = r#"{"owner":{"rid":"hallway","rtype":"room"},
         "motion":{"motion_report":{"changed":"2026-09-03T17:20:09.413Z","motion":false}}}"#;
    for (order, (first, second)) in [
        ("studio first", (studio, hallway)),
        ("hallway first", (hallway, studio)),
    ] {
        assert_eq!(
            reading(
                &format!("{{\"data\":[{first},{second}]}}"),
                ROOMS,
                &watched(),
                1_788_456_100
            )
            .and_then(|raw| raw.edge),
            Some(Edge {
                epoch: 1_788_456_009,
                motion: true,
                room: "3F - Studio".to_string(),
            }),
            "listed {order}, the tie went to the listing rather than to the reading"
        );
    }
}
