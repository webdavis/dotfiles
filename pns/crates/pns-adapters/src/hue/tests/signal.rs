//! The hue channel, pinned: signal.

use super::fixtures::*;

// --- the sequence -------------------------------------------------------

fn pulse() -> HuePulse<ScriptedBridge> {
    HuePulse {
        bridge: bridge(),
        rooms: wanted(&["3F - Studio", "2F - Kitchen"]),
    }
}

// --- the signal ---------------------------------------------------------

const RED_SIGNAL: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.675,"y":0.322}}],"duration":3000,"signal":"on_off_color"}}"#;
const GREEN_SIGNAL: &str = r#"{"signaling":{"colors":[{"xy":{"x":0.17,"y":0.7}}],"duration":3000,"signal":"on_off_color"}}"#;

#[test]
fn a_failure_signals_every_wanted_room_red_and_writes_nothing_else() {
    let hue = pulse();
    hue.run(Behaviour::Failed);
    assert_eq!(
        hue.bridge.puts.borrow().as_slice(),
        &[
            ("grouped_light/grp-1".to_string(), RED_SIGNAL.to_string()),
            ("grouped_light/grp-2".to_string(), RED_SIGNAL.to_string()),
        ],
        "one signal per wanted room, and no restore writes: the bridge owns the restore. \
         Independence is structural now: put reports nothing, so no outcome can \
         short-circuit the room behind it"
    );
    assert_eq!(
        hue.bridge.gets.borrow().as_slice(),
        &["room".to_string()],
        "the light inventory is never fetched on the no-map path"
    );
}

#[test]
fn the_no_map_body_states_no_brightness_and_keeps_its_own_duration() {
    // THE COMPATIBILITY CLAIM, pinned. This is the request a machine with no
    // `[lights]` table sends, and it must not gain a `dimming` field: there
    // is no routing in reach to dim, so a brightness stated here would take
    // a level the operator set by hand and hold the room at it for good.
    // Its three-second duration is deliberately NOT the routed path's
    // locked four: that figure was locked where a knob states it.
    let hue = pulse();
    hue.run(Behaviour::Done);
    let puts = hue.bridge.puts.borrow();
    assert_eq!(puts.len(), 2);
    assert_eq!(puts[0].1, GREEN_SIGNAL);
    assert!(!puts[0].1.contains("dimming"));
}

#[test]
fn a_room_the_bridge_does_not_have_is_skipped_in_silence() {
    let hue = HuePulse {
        bridge: bridge(),
        rooms: wanted(&["3F - Studio", "1F - Renamed Away"]),
    };
    hue.run(Behaviour::Failed);
    let puts = hue.bridge.puts.borrow();
    assert_eq!(
        puts.len(),
        1,
        "only the room that still exists is signalled"
    );
    assert_eq!(puts[0].0, "grouped_light/grp-1");
}

#[test]
fn no_matching_rooms_or_no_lights_is_a_silent_no_op() {
    let hue = HuePulse {
        bridge: scripted(Some(r#"{"data":[]}"#)),
        rooms: wanted(&["3F - Studio"]),
    };
    hue.run(Behaviour::Done);
    assert!(hue.bridge.puts.borrow().is_empty());
}

#[test]
fn a_held_state_has_no_room_shaped_body_so_the_no_map_pulse_writes_nothing() {
    // A BEHAVIOUR WITH NO PULSE SHAPE WRITES NOTHING AT ALL rather than
    // falling back to one that has one. A lamp asked to breathe would
    // otherwise flash whatever shape was nearest, which is the lying lamp
    // this whole design exists to prevent.
    for held in [Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping] {
        let hue = pulse();
        assert_eq!(hue.run(held), 0, "{held:?} has no room-shaped body");
        assert!(hue.bridge.puts.borrow().is_empty());
    }
}

#[test]
fn the_pulse_reports_how_many_rooms_it_signalled() {
    // THE ONLY THING ABOUT THE LIGHTS ANYONE CAN CHECK. The bridge owns
    // the whole effect and acknowledges no write, so the count of rooms
    // that were signalled is the last observable fact on this path; zero
    // is the shape every hue misconfiguration takes.
    assert_eq!(
        pulse().run(Behaviour::Done),
        2,
        "one signal per matched room"
    );
    assert_eq!(
        HuePulse {
            bridge: scripted(None),
            rooms: wanted(&["3F - Studio"]),
        }
        .run(Behaviour::Done),
        0,
        "a bridge that answered no listing signalled nothing"
    );
    assert_eq!(
        HuePulse {
            bridge: bridge(),
            rooms: wanted(&["1F - Renamed Away"]),
        }
        .run(Behaviour::Done),
        0,
        "and neither did a listing in which no configured name matched"
    );
}
