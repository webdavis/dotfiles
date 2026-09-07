use super::*;

// --- reading the mode catalog -------------------------------------------

#[test]
fn the_catalog_turns_an_identifier_into_the_name_control_center_shows() {
    let names = mode_names(LIVE_CATALOG);
    assert_eq!(
        names.get(CASUALLY_CONCERNED).map(String::as_str),
        Some("Casually Concerned")
    );
    assert_eq!(
        names.get("com.apple.sleep.sleep-mode").map(String::as_str),
        Some("Sleep")
    );
}

#[test]
fn a_mode_is_named_by_its_own_identifier_field_and_never_by_the_map_key() {
    // THE MAP KEY IS A CONVENTION APPLE DOCUMENTS NOWHERE, and only the
    // field is the one an assertion's `assertionDetailsModeIdentifier` is
    // named after. Every entry on this machine spells the two the same, so
    // this synthetic pair is the only thing standing between the choice
    // and a reader that keyed on the map key and passed anyway.
    let names = mode_names(LIVE_CATALOG);
    assert_eq!(
        names.get(KEY_DISAGREES).map(String::as_str),
        Some("Straße"),
        "the record's own field is the key"
    );
    assert!(
        !names.contains_key("com.apple.donotdisturb.mode.a-key-that-disagrees"),
        "and the map key it sat under is not: {names:?}"
    );
}

#[test]
fn a_catalog_nothing_can_read_resolves_no_names_at_all() {
    // FAIL OPEN AGAIN, and in the same direction: with no names, only a
    // raw identifier in the config can match, so a broken catalog silences
    // less rather than more.
    for shape in ["", "{", "null", r#"{"data":[]}"#, r#"{"data":[{}]}"#] {
        assert!(
            mode_names(shape).is_empty(),
            "an unreadable catalog names nothing: {shape}"
        );
    }
}
