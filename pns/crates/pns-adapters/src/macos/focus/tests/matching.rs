use super::*;

// --- the policy ---------------------------------------------------------

#[test]
fn a_mode_the_config_names_by_its_display_name_is_silenced() {
    assert!(silenced(
        &active_modes(LIVE_ACTIVE),
        &mode_names(LIVE_CATALOG),
        &named(&["Casually Concerned"])
    ));
}

#[test]
fn the_name_is_matched_however_the_operator_capitalised_it() {
    // Transcribed by hand out of a user interface: "sleep" is not a
    // different Focus from "Sleep".
    assert!(silenced(
        &active_modes(LIVE_ACTIVE),
        &mode_names(LIVE_CATALOG),
        &named(&["casually CONCERNED"])
    ));
}

#[test]
fn a_name_whose_lowercase_disagrees_with_itself_is_still_the_same_name() {
    // MEASURED, and the reason `same` folds both ways: "Straße"
    // lowercases to "straße" and "STRASSE" lowercases to "strasse", so a
    // lowercase-only compare reads a name the operator typed in capitals
    // as a different Focus and silently silences nothing.
    assert!(silenced(
        &asserted(&[KEY_DISAGREES]),
        &mode_names(LIVE_CATALOG),
        &named(&["STRASSE"])
    ));
}

#[test]
fn a_raw_mode_identifier_is_accepted_for_a_mode_the_catalog_does_not_name() {
    // The identifier is the ONLY handle an unnamed mode has, so it is
    // matched with no catalog at all.
    assert!(silenced(
        &active_modes(LIVE_ACTIVE),
        &BTreeMap::new(),
        &named(&[CASUALLY_CONCERNED])
    ));
}

#[test]
fn a_focus_nobody_named_silences_nothing() {
    // THE WHOLE POINT OF PER-MODE POLICY. A Focus was asserted for 95% of
    // one measured day on this machine, so "a Focus is on" is not a
    // question worth acting on; "is it one of these" is.
    assert!(!silenced(
        &active_modes(LIVE_ACTIVE),
        &mode_names(LIVE_CATALOG),
        &named(&["Sleep", "Coding"])
    ));
}

#[test]
fn an_empty_list_is_the_feature_switched_off() {
    assert!(!silenced(
        &active_modes(LIVE_ACTIVE),
        &mode_names(LIVE_CATALOG),
        &[]
    ));
}

#[test]
fn a_named_mode_that_is_not_asserted_silences_nothing() {
    assert!(!silenced(
        &asserted(&[]),
        &mode_names(LIVE_CATALOG),
        &named(&["Casually Concerned"])
    ));
}
