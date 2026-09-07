//! The hue channel, pinned: dim.

use super::fixtures::*;

// --- the dim window ------------------------------------------------------

/// 22:00 to 07:00, which is the window every room in the operator's own
/// config carries.
fn night(behaviours: &[Behaviour]) -> DimWindow {
    DimWindow {
        window: parse_window("22:00-07:00").expect("a window the parser takes"),
        behaviours: behaviours.to_vec(),
    }
}

const MIDNIGHT: Option<u16> = Some(0);
const NOON: Option<u16> = Some(12 * 60);

#[test]
fn inside_a_window_an_enabled_behaviour_runs_dim_and_one_that_is_not_is_suppressed() {
    let window = night(&[Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping]);
    assert_eq!(
        dim_showing(Some(&window), Behaviour::Blocked, MIDNIGHT),
        Showing::Dimmed,
        "an enabled behaviour runs its dim form"
    );
    assert_eq!(
        dim_showing(Some(&window), Behaviour::Done, MIDNIGHT),
        Showing::Dark,
        "and one the operator did not enable is taken away entirely: no strobes \
         while they are asleep"
    );
    assert_eq!(
        dim_showing(Some(&window), Behaviour::Done, NOON),
        Showing::Full,
        "outside the window everything runs full"
    );
    assert_eq!(
        dim_showing(None, Behaviour::Done, MIDNIGHT),
        Showing::Full,
        "and a lamp with no window at all is untouched at every hour, which is \
         what makes the whole feature opt-in"
    );
}

#[test]
fn a_window_with_nothing_enabled_suppresses_every_behaviour_and_needs_no_mode() {
    // THE BEDROOM RULE, and it needs no special case in the code: the
    // operator's "never any light behaviour in here during quiet hours" is a
    // window with an empty enable list, which is already what an empty list
    // means everywhere else.
    let window = night(&[]);
    for behaviour in [
        Behaviour::Done,
        Behaviour::Failed,
        Behaviour::Blocked,
        Behaviour::Unread,
        Behaviour::Looping,
    ] {
        assert_eq!(
            dim_showing(Some(&window), behaviour, MIDNIGHT),
            Showing::Dark,
            "{behaviour:?} is suppressed by a window that enables nothing"
        );
        assert_eq!(
            dim_showing(Some(&window), behaviour, NOON),
            Showing::Full,
            "{behaviour:?} is untouched outside it"
        );
    }
}

#[test]
fn a_clock_this_machine_cannot_read_is_treated_as_inside_the_window() {
    // FAIL CLOSED, which is `quiet_now`'s own direction: a flash at 3am is
    // what the window was set to prevent, and a missed signal costs nothing.
    assert_eq!(
        dim_showing(Some(&night(&[])), Behaviour::Done, None),
        Showing::Dark
    );
}

#[test]
fn a_dim_window_nobody_can_parse_leaves_that_lamp_dark_and_says_which_lamp() {
    // FAIL CLOSED FOR THAT LAMP ALONE. An operator who asked for a dim
    // window and mistyped it would otherwise be flashed at 3am and told
    // nothing; the cost of the refusal is one lamp rather than the house.
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             dim_window = \"2200-0700\"\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"done\"]\n",
        ),
    );
    assert_eq!(
        routing.refusals,
        vec![
            "lights: `3F - Studio - HCL1` has dim_window \"2200-0700\", which is not \
             a HH:MM-HH:MM window; that lamp stays dark"
                .to_string(),
            "lights: `3F - Studio - HCL3` has dim_window \"2200-0700\", which is not \
             a HH:MM-HH:MM window; that lamp stays dark"
                .to_string(),
            "lights: `3F - Studio - HCL2` has dim_window \"2200-0700\", which is not \
             a HH:MM-HH:MM window; that lamp stays dark"
                .to_string(),
        ],
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL1"),
        None,
        "the lamp is dark rather than signalling at an hour nobody could judge"
    );
    assert_eq!(
        carried(&routing, "2F - Kitchen - HCD3"),
        Some(vec![Behaviour::Done]),
        "and a lamp inheriting a readable declaration keeps its behaviours"
    );
}

// --- the mute ------------------------------------------------------------

#[test]
fn a_mute_reaches_a_lamp_by_its_own_name_by_its_room_and_by_any_zone_holding_it() {
    let hcl1 = stock()
        .lamps
        .into_iter()
        .find(|lamp| lamp.name == "3F - Studio - HCL1")
        .expect("HCL1 is in the listing");
    for typed in ["3F - Studio - HCL1", "3F - Studio", "Upstairs", "Desk"] {
        assert!(
            muted_now(&hcl1, &Muting::Places(vec![typed.to_string()])),
            "{typed:?} must reach this lamp"
        );
    }
    assert!(
        !muted_now(&hcl1, &Muting::Places(vec!["2F - Kitchen".to_string()])),
        "and a name it does not answer to reaches nothing"
    );
    assert!(
        !muted_now(&hcl1, &Muting::Places(Vec::new())),
        "an empty mute list mutes nothing"
    );
    // AND A READING NOBODY COULD TAKE MUTES EVERY LAMP, which is the fail
    // direction on a lamp path: an unreadable mute record or clock must not
    // arrive here as a house with nothing quiet in it.
    assert!(
        muted_now(&hcl1, &Muting::Everything),
        "a mute nobody could read leaves every lamp quiet, never loud"
    );
}

#[test]
fn the_names_a_mute_takes_are_the_declarations_and_the_bridges_own_three_levels() {
    let declared = lights(
        "[lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"blocked\"]\n\
         [lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
         [lights.zone.Upstairs]\nshows = [\"done\"]\n",
    );
    assert_eq!(
        mutable_names(&declared, None),
        vec![
            "3F - Studio".to_string(),
            "3F - Studio - HCL3".to_string(),
            "Upstairs".to_string(),
        ],
        "sorted, deduplicated, and with no level of its own: a mute names a place, \
         and every level is one. A bridge that answered nothing leaves these, \
         which are the names a mute can still enforce once it is back"
    );
    // AND THE BRIDGE'S OWN NAMES ARE IN THE GRAMMAR TOO. The targets are
    // lamp, room and zone, not "whatever the config wrote down": a lamp
    // inheriting its room's declaration is a real name the operator reads
    // off the bridge's own app, and refusing it sent them away from the room
    // they were standing in at the hour they wanted it quiet.
    let both = mutable_names(&declared, Some(&stock()));
    for reachable in [
        "3F - Studio - HCL1",
        "3F - Studio - HCL3",
        "2F - Kitchen",
        "Upstairs",
    ] {
        assert!(
            both.contains(&reachable.to_string()),
            "{reachable:?} is a name a mute reaches: {both:?}"
        );
    }
    assert!(
        both.windows(2).all(|pair| pair[0] < pair[1]),
        "still sorted and deduplicated across both sources: {both:?}"
    );
    assert!(mutable_names(&lights("[lights]\n"), None).is_empty());
}
