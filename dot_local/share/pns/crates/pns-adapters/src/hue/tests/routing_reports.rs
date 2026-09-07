use super::fixtures::*;

#[test]
fn an_empty_behaviour_set_leaves_a_lamp_out_rather_than_writing_to_it() {
    // A DELIBERATE EMPTY DECLARATION IS AN OVERRIDE, and it has to beat the
    // room: it is how one lamp in a routed room is taken out of service.
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.lamp.\"3F - Studio - HCL3\"]\nshows = []\n",
        ),
    );
    assert_eq!(carried(&routing, "3F - Studio - HCL3"), None);
    assert_eq!(routing.lamps.len(), 2, "the room's other two lamps remain");
}

#[test]
fn a_name_the_bridge_does_not_have_is_reported_with_the_level_that_wrote_it() {
    let routing = resolve(
        &stock(),
        &lights(
            "[lights.lamp.\"3F - Studio - HCL9\"]\nshows = [\"done\"]\n\
             [lights.room.\"3F - Attic\"]\nshows = [\"done\"]\n\
             [lights.room.\"3F - Cupboard\"]\nshows = [\"done\"]\n\
             [lights.zone.Outdoors]\nshows = [\"done\"]\n",
        ),
    );
    assert_eq!(
        routing.unresolved,
        vec![
            Unresolved {
                level: "lamp".to_string(),
                name: "3F - Studio - HCL9".to_string(),
                kind: Missing::NotOnBridge,
            },
            Unresolved {
                level: "room".to_string(),
                name: "3F - Attic".to_string(),
                kind: Missing::NotOnBridge,
            },
            Unresolved {
                level: "room".to_string(),
                name: "3F - Cupboard".to_string(),
                kind: Missing::AddressedNothing,
            },
            Unresolved {
                level: "zone".to_string(),
                name: "Outdoors".to_string(),
                kind: Missing::AddressedNothing,
            },
        ],
        "a typo and an empty room are DIFFERENT sentences, because an operator \
         sent looking for a room sitting in front of them acts on a lie"
    );
    assert!(routing.lamps.is_empty());
}

#[test]
fn a_case_folded_name_is_a_typo_rather_than_a_name_to_forgive() {
    // WHICH IS HOW THE BRIDGE READS IT TOO. Forgiving case here would make
    // the routing depend on a rule the bridge's own listing does not follow.
    let routing = resolve(
        &stock(),
        &lights("[lights.room.\"3f - studio\"]\nshows = [\"done\"]\n"),
    );
    assert_eq!(routing.unresolved.len(), 1);
    assert_eq!(routing.unresolved[0].kind, Missing::NotOnBridge);
    assert!(routing.lamps.is_empty());
}

#[test]
fn a_lamp_moved_to_another_room_answers_the_room_it_is_in_now() {
    // THE BRIDGE'S CURRENT MEMBERSHIP IS THE TRUTH AT RESOLVE TIME, which is
    // the case the whole join exists for: a lamp physically moved is not a
    // config to edit. HCL3's device leaves the studio's children and joins
    // the kitchen's, which is what the listing shows after the operator
    // drags a lamp between rooms in the app.
    let moved = CLIP_ROOMS
        .replace(
            r#"{"rid":"c97b44a9-cdcc-48c3-a15d-630fdaa936d0","rtype":"device"},"#,
            "",
        )
        .replace(
            r#""children":[{"rid":"b1e78057-aa81-4de0-ab08-6d06e1736dd6","rtype":"device"}]"#,
            r#""children":[{"rid":"b1e78057-aa81-4de0-ab08-6d06e1736dd6","rtype":"device"},
                   {"rid":"c97b44a9-cdcc-48c3-a15d-630fdaa936d0","rtype":"device"}]"#,
        );
    let held = inventory(&moved, CLIP_LIGHTS, CLIP_ZONES);
    assert_eq!(
        held.lamps
            .iter()
            .find(|lamp| lamp.name == "3F - Studio - HCL3")
            .and_then(|lamp| lamp.room.clone()),
        Some("2F - Kitchen".to_string()),
        "the join reads the room the lamp is in NOW, whatever its name says"
    );
    let routing = resolve(
        &held,
        &lights(
            "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"blocked\"]\n",
        ),
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL3"),
        Some(vec![Behaviour::Blocked]),
        "so it answers its NEW room's declaration and no longer the old one"
    );
    assert_eq!(
        carried(&routing, "3F - Studio - HCL1"),
        Some(vec![Behaviour::Done])
    );
}

#[test]
fn every_listing_is_fetched_and_a_bridge_that_refused_one_resolves_nothing() {
    // A LISTING THAT FAILED AND A LISTING THAT WAS EMPTY ARE DIFFERENT
    // ANSWERS. Collapsing them would resolve a config against an empty
    // inventory: every name reported as a typo, every lamp dark, all of it
    // stated confidently about a bridge that said nothing.
    let full = ScriptedBridge {
        rooms: Some(CLIP_ROOMS),
        lights: Some(CLIP_LIGHTS),
        zones: Some(CLIP_ZONES),
        gets: RefCell::new(Vec::new()),
        puts: RefCell::new(Vec::new()),
    };
    let map = resolve_on_bridge(
        &full,
        &lights("[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n"),
    );
    assert_eq!(map.map(|routing| routing.lamps.len()), Some(3));
    assert_eq!(
        full.gets.borrow().as_slice(),
        &["room".to_string(), "light".to_string(), "zone".to_string()],
    );
    let no_zones = ScriptedBridge {
        rooms: Some(CLIP_ROOMS),
        lights: Some(CLIP_LIGHTS),
        zones: None,
        gets: RefCell::new(Vec::new()),
        puts: RefCell::new(Vec::new()),
    };
    assert!(
        resolve_on_bridge(
            &no_zones,
            &lights("[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n")
        )
        .is_none(),
        "one refused listing resolves NOTHING rather than everything else"
    );
}
