//! The hue channel, pinned: settings.

use super::fixtures::*;

// --- settings -----------------------------------------------------------

/// A pin every armed fixture carries, so the settings tests state one fact
/// each: the pin's own behaviour is pinned by the tests below it.
const PIN: &str =
    "certificate = \"sha256:0000000000000000000000000000000000000000000000000000000000000001\"";

#[test]
fn a_bridge_and_key_are_required_and_their_absence_is_silence() {
    assert_eq!(hue_settings(&table(""), None), Ok(None));
    assert_eq!(
        hue_settings(&table(r#"bridge = "192.168.1.10""#), None),
        Ok(None)
    );
    assert_eq!(hue_settings(&table(r#"key = "k""#), None), Ok(None));
    assert_eq!(
        hue_settings(&table("bridge = \"192.168.1.10\"\nkey = \"\""), None),
        Ok(None)
    );
}

#[test]
fn an_armed_table_with_no_certificate_is_refused_naming_the_key_and_the_command() {
    let refusal =
        hue_settings(&table("bridge = \"b\"\nkey = \"k\""), None).expect_err("a config refusal");
    assert!(refusal.contains("plugins.hue.certificate"), "{refusal}");
    assert!(refusal.contains("pns lights enroll"), "{refusal}");
    assert!(refusal.contains("no pulse"), "{refusal}");
}

#[test]
fn a_malformed_certificate_is_refused_quoting_what_was_written() {
    let refusal = hue_settings(
        &table("bridge = \"b\"\nkey = \"k\"\ncertificate = \"sha256:nope\""),
        None,
    )
    .expect_err("a config refusal");
    assert!(refusal.contains("plugins.hue.certificate"), "{refusal}");
    assert!(refusal.contains("sha256:nope"), "{refusal}");
}

#[test]
fn an_empty_certificate_is_the_same_refusal_as_an_absent_one() {
    let absent = hue_settings(&table("bridge = \"b\"\nkey = \"k\""), None).expect_err("a refusal");
    let empty = hue_settings(
        &table("bridge = \"b\"\nkey = \"k\"\ncertificate = \"\""),
        None,
    )
    .expect_err("a refusal");
    assert_eq!(absent, empty);
}

#[test]
fn rooms_default_to_the_bash_pair_when_nothing_names_them() {
    let settings = hue_settings(&table(&format!("bridge = \"b\"\nkey = \"k\"\n{PIN}")), None)
        .unwrap()
        .unwrap();
    assert_eq!(settings.rooms, DEFAULT_ROOMS.to_vec());
}

#[test]
fn the_environment_override_wins_and_splits_on_newlines() {
    let settings = hue_settings(
        &table(&format!(
            "bridge = \"b\"\nkey = \"k\"\nrooms = [\"Config Room\"]\n{PIN}"
        )),
        Some("Room A\nRoom B\n"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(settings.rooms, vec!["Room A", "Room B"]);
}

#[test]
fn the_settings_rooms_array_beats_the_defaults() {
    let settings = hue_settings(
        &table(&format!(
            "bridge = \"b\"\nkey = \"k\"\nrooms = [\"Config Room\"]\n{PIN}"
        )),
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(settings.rooms, vec!["Config Room"]);
}

#[test]
fn a_refusal_reaches_the_caller_of_armed_hue_and_the_settings_do_not() {
    let mut said = String::new();
    let armed = super::super::armed_hue(&table("bridge = \"b\"\nkey = \"k\""), None, |line| {
        said = line.to_string();
    });
    assert!(armed.is_none());
    assert!(said.contains("plugins.hue.certificate"), "{said}");
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
