//! The hue channel, pinned: settings.

use super::fixtures::*;

// --- settings -----------------------------------------------------------

#[test]
fn a_bridge_and_key_are_required_and_their_absence_is_silence() {
    assert_eq!(hue_settings(&table(""), None), None);
    assert_eq!(
        hue_settings(&table(r#"bridge = "192.168.1.10""#), None),
        None
    );
    assert_eq!(hue_settings(&table(r#"key = "k""#), None), None);
    assert_eq!(
        hue_settings(&table("bridge = \"192.168.1.10\"\nkey = \"\""), None),
        None
    );
}

#[test]
fn rooms_default_to_the_bash_pair_when_nothing_names_them() {
    let settings = hue_settings(&table("bridge = \"b\"\nkey = \"k\""), None).unwrap();
    assert_eq!(settings.rooms, DEFAULT_ROOMS.to_vec());
}

#[test]
fn the_environment_override_wins_and_splits_on_newlines() {
    let settings = hue_settings(
        &table("bridge = \"b\"\nkey = \"k\"\nrooms = [\"Config Room\"]"),
        Some("Room A\nRoom B\n"),
    )
    .unwrap();
    assert_eq!(settings.rooms, vec!["Room A", "Room B"]);
}

#[test]
fn the_settings_rooms_array_beats_the_defaults() {
    let settings = hue_settings(
        &table("bridge = \"b\"\nkey = \"k\"\nrooms = [\"Config Room\"]"),
        None,
    )
    .unwrap();
    assert_eq!(settings.rooms, vec!["Config Room"]);
}

// --- the CLIP parsers ---------------------------------------------------

#[test]
fn a_room_without_a_grouped_light_is_skipped_whole() {
    const NO_GROUP: &str = r#"{"data":[
          {"id":"room-1","type":"room","metadata":{"name":"Groupless"},
           "children":[{"rid":"dev-a","rtype":"device"}],"services":[]}
        ]}"#;
    assert!(grouped_light_ids_for_rooms(NO_GROUP, &wanted(&["Groupless"])).is_empty());
}

#[test]
fn a_renamed_room_is_skipped_and_unparseable_json_is_empty() {
    assert!(grouped_light_ids_for_rooms(ROOMS_JSON, &wanted(&["Gone Room"])).is_empty());
    assert!(grouped_light_ids_for_rooms("not json", &wanted(&["3F - Studio"])).is_empty());
}
