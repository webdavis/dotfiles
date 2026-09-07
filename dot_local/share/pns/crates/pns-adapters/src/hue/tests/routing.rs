//! The hue channel, pinned: routing.

use super::fixtures::*;

// --- the inventory and the routing grammar -------------------------------

const HCL1: &str = "17295316-360e-4259-b8fd-928caf1f9c3e";

#[test]
fn a_lamp_knows_its_own_name_its_room_and_every_zone_holding_it() {
    let held = stock();
    let hcl1 = held
        .lamps
        .iter()
        .find(|lamp| lamp.name == "3F - Studio - HCL1")
        .expect("HCL1 is in the listing");
    assert_eq!(hcl1.id, HCL1);
    assert_eq!(
        hcl1.room.as_deref(),
        Some("3F - Studio"),
        "the ROOM join runs through the lamp's owning device"
    );
    assert_eq!(
        hcl1.zones,
        vec!["Upstairs".to_string(), "Desk".to_string()],
        "and the ZONE join reads the lights a zone lists DIRECTLY, which is a \
         different shape: one join for both would leave every zone empty"
    );
    let kitchen = held
        .lamps
        .iter()
        .find(|lamp| lamp.name == "2F - Kitchen - HCD3")
        .expect("the kitchen lamp is in the listing");
    assert_eq!(kitchen.room.as_deref(), Some("2F - Kitchen"));
    assert!(
        kitchen.zones.is_empty(),
        "a lamp no zone holds carries no zone name"
    );
    assert_eq!(held.lamps.len(), 4, "the dimmer switch owns no light");
    assert!(held.rooms.contains(&"3F - Cupboard".to_string()));
    assert!(held.zones.contains(&"Outdoors".to_string()));
}

#[test]
fn the_most_specific_declaration_that_names_a_lamp_supplies_its_whole_behaviour_set() {
    // THE OPERATOR'S OWN ROUTING, in miniature: the room carries the pulses
    // and one lamp inside it is lifted out for the held states. A UNION
    // would re-add exactly what the lamp-level declaration left out, which
    // is why levels never merge.
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.room.\"3F - Studio\"]\nshows = [\"done\", \"failed\"]\n\
             [lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"blocked\", \"unread\"]\n",
        ),
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL3"),
        Some(vec![Behaviour::Blocked, Behaviour::Unread]),
        "the lamp's own declaration wins outright and the room's does not merge in"
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL1"),
        Some(vec![Behaviour::Done, Behaviour::Failed]),
        "and every other lamp in the room still takes the room's"
    );
    assert_eq!(
        carried(&routing, "2F - Kitchen - HCD3"),
        None,
        "a lamp no declaration names carries nothing"
    );
}

#[test]
fn a_room_beats_a_zone_and_a_zone_answers_a_lamp_no_nearer_declaration_names() {
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.zone.Upstairs]\nshows = [\"loop\"]\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n",
        ),
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL1"),
        Some(vec![Behaviour::Done]),
        "a room is more specific than a zone, so the zone never answers here"
    );
    let zone_only = resolve(
        &stock(),
        &lights("[lights.zone.Upstairs]\nshows = [\"loop\"]\n"),
    );
    assert_eq!(
        carried(&zone_only, "3F - Studio - HCL1"),
        Some(vec![Behaviour::Looping]),
        "with nothing nearer, the zone is what answers"
    );
    assert_eq!(
        carried(&zone_only, "3F - Studio - HCL3"),
        None,
        "and a lamp that zone does not hold is untouched by it"
    );
}

#[test]
fn a_lamp_two_zones_both_answer_for_is_refused_with_both_named() {
    // THERE IS NO SPECIFICITY TO ARBITRATE between two zones, and guessing
    // is against house style, so the question answers NOTHING for that lamp
    // and the operator is told which two declarations to break the tie
    // between.
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.zone.Upstairs]\nshows = [\"loop\"]\n\
             [lights.zone.Desk]\nshows = [\"blocked\"]\n",
        ),
    );
    assert_eq!(
        routing.refusals,
        vec![
            "lights: `3F - Studio - HCL1` is covered by 2 zone declarations that \
             each state `shows` (\"Desk\" and \"Upstairs\"); there is nothing more \
             specific to break the tie, so that lamp answers none of them"
                .to_string()
        ],
        "both declarations are cited by name"
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL1"),
        None,
        "and the contested lamp carries nothing rather than one of the two"
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL2"),
        Some(vec![Behaviour::Looping]),
        "a lamp only ONE of them holds is unaffected: the refusal is per lamp"
    );
}

#[test]
fn a_dim_question_two_zones_both_answer_leaves_that_lamp_dark_rather_than_bright() {
    // THE FAIL DIRECTION ON A LAMP PATH IS DARK, and a refusal used to
    // arrive at the caller as the same `None` as "nobody said anything
    // about quiet hours". So the one config that says LOUDEST that a lamp
    // must be quiet at night, two declarations both stating when, was the
    // config that ran it at full brightness all night.
    //
    // THE SAME DIRECTION AN UNPARSEABLE WINDOW ALREADY TAKES: that lamp
    // drops out of the routing entirely, which costs one lamp rather than
    // the house, and the refusal names both declarations.
    // THE ROOM ANSWERS `shows`, so the lamp really is routed and the ONLY
    // question in doubt is the dim one: a config where both were contested
    // would drop the lamp for carrying nothing and prove nothing here.
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.room.\"3F - Studio\"]\nshows = [\"loop\"]\n\
             [lights.zone.Upstairs]\n\
             dim_window = \"22:00-07:00\"\ndim_behaviours = [\"loop\"]\n\
             [lights.zone.Desk]\n\
             dim_window = \"23:00-06:00\"\ndim_behaviours = [\"loop\"]\n",
        ),
    );
    assert!(
        routing.refusals.iter().any(|refusal| refusal
            .contains("is covered by 2 zone declarations that each state `dim_window`")),
        "both declarations are cited by name: {:?}",
        routing.refusals
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL1"),
        None,
        "the lamp whose quiet hours nobody can settle is dark, never full"
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL2"),
        Some(vec![Behaviour::Looping]),
        "a lamp only ONE of them holds is unaffected: the refusal is per lamp"
    );
}

#[test]
fn each_question_resolves_on_its_own_so_a_lamp_can_state_one_and_inherit_the_other() {
    // PER QUESTION, NOT WHOLESALE. A lamp that says which behaviours it
    // carries and nothing about quiet hours still takes its room's window;
    // an entry-shaped chain would have taken that away the moment the lamp
    // wrote one key.
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             dim_window = \"22:00-07:00\"\ndim_behaviours = [\"blocked\"]\n\
             [lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"blocked\"]\n",
        ),
    );
    let hcl3 = routing
        .lamps
        .iter()
        .find(|routed| routed.lamp.name == "3F - Studio - HCL3")
        .expect("HCL3 is routed");
    assert_eq!(hcl3.shows, vec![Behaviour::Blocked], "its own answer");
    assert_eq!(
        hcl3.dim.as_ref().map(|dim| dim.behaviours.clone()),
        Some(vec![Behaviour::Blocked]),
        "and its room's window, because the lamp said nothing about quiet hours"
    );
}
