use super::*;

// --- the lights section --------------------------------------------------

/// A routing with three lamps, each carrying a different behaviour set, so
/// the per-behaviour counts below can differ from each other.
fn routing() -> pns_hue::Routing {
    let lamp = |name: &str| pns_hue::Lamp {
        id: format!("id-{name}"),
        name: name.to_string(),
        room: None,
        zones: Vec::new(),
    };
    pns_hue::Routing {
        lamps: vec![
            pns_hue::Routed {
                lamp: lamp("HCL1"),
                shows: vec![Behaviour::Done, Behaviour::Failed],
                dim: None,
            },
            pns_hue::Routed {
                lamp: lamp("HCL2"),
                shows: vec![Behaviour::Done, Behaviour::Failed],
                dim: None,
            },
            pns_hue::Routed {
                lamp: lamp("HCL3"),
                shows: vec![Behaviour::Blocked, Behaviour::Unread],
                dim: None,
            },
        ],
        unresolved: Vec::new(),
        refusals: Vec::new(),
    }
}

#[test]
fn the_lights_section_says_which_of_its_six_states_the_config_is_in() {
    assert_eq!(
        lights_lines(&LightsReport::Off),
        vec![
            "pns doctor: lights: off in the config, so the pulse uses the \
                 [plugins.hue] rooms"
        ],
        "no table is the state every machine was in before this table existed"
    );
    assert_eq!(
        lights_lines(&LightsReport::HueMissing),
        vec![
            "pns doctor: lights: configured, but there is no [plugins.hue] \
                 table to light them through"
        ],
        "A TABLE THAT WAS NEVER WRITTEN IS NOT A SWITCH SOMEONE TURNED OFF. \
             One is a config that is half finished and the other is a decision, \
             and telling an operator to go flip a switch that is not there is the \
             kind of wrong direction they act on"
    );
    assert_eq!(
        lights_lines(&LightsReport::HueDisabled),
        vec![
            "pns doctor: lights: configured, but [plugins.hue] enabled is false, \
                 so nothing lights"
        ],
        "ONE SWITCH, and the doctor is where an operator sees it is off"
    );
    assert_eq!(
        lights_lines(&LightsReport::NoBridge),
        vec![
            "pns doctor: lights: no [plugins.hue] bridge and key, so no lamp \
                 could be resolved"
        ],
        "a config that named no bridge is not a bridge that answered nothing"
    );
    assert_eq!(
        lights_lines(&LightsReport::Unreachable),
        vec!["pns doctor: lights: the bridge listed nothing, so no lamp resolved"],
        "a bridge that answered nothing is not a config that named nothing"
    );
    assert_eq!(
        lights_lines(&LightsReport::Resolved(routing())),
        vec!["pns doctor: lights: done 2, failed 2, blocked 1, unread 1, loop 0"],
        "PER BEHAVIOUR, which is the question an operator opens this section \
             with: did the thing I routed reach a bulb. A behaviour nothing carries \
             is listed at zero rather than left out, because an absence reads as fine"
    );
}

#[test]
fn an_unresolved_name_and_a_refused_declaration_each_get_their_own_line() {
    let mut map = routing();
    map.unresolved = vec![
        pns_hue::Unresolved {
            level: "lamp".to_string(),
            name: "3F - Studio - HCL9".to_string(),
            kind: pns_hue::Missing::NotOnBridge,
        },
        pns_hue::Unresolved {
            level: "room".to_string(),
            name: "3F - Cupboard".to_string(),
            kind: pns_hue::Missing::AddressedNothing,
        },
    ];
    map.refusals = vec!["lights: `HCL1` is covered by 2 zone declarations".to_string()];
    assert_eq!(
        lights_lines(&LightsReport::Resolved(map)),
        vec![
            "pns doctor: lights: done 2, failed 2, blocked 1, unread 1, loop 0",
            "pns doctor: lights: `3F - Studio - HCL9` (lamp) is not on the bridge",
            "pns doctor: lights: `3F - Cupboard` (room) is on the bridge, but it \
                 holds no lamp",
            "pns doctor: lights: `HCL1` is covered by 2 zone declarations",
        ],
        "every miss is named with the level that wrote it, in the words of the \
             miss it actually was, and every refusal in the channel's own words"
    );
}

#[test]
fn every_lights_state_says_something_rather_than_printing_nothing() {
    // WHAT THIS PINS, and only this: a section that reports and never
    // grades has one way to fail silently, which is a state that produces
    // no line at all, leaving the operator to read an absence as "fine".
    for report in [
        LightsReport::Off,
        LightsReport::HueMissing,
        LightsReport::HueDisabled,
        LightsReport::NoBridge,
        LightsReport::Unreachable,
        LightsReport::Resolved(routing()),
        LightsReport::Resolved(pns_hue::Routing::default()),
    ] {
        assert!(
            !lights_lines(&report).is_empty(),
            "every state says something, the empty map included"
        );
    }
}
