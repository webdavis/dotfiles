use super::*;

// --- the Focus modes that mean it ---------------------------------------

#[test]
fn a_focus_table_names_the_modes_that_silence_pns() {
    let config = parse_config("[focus]\nsilence = [\"Sleep\", \"Coding\"]\n").unwrap();
    assert_eq!(config.focus_silence, ["Sleep", "Coding"]);
}

#[test]
fn a_config_with_no_focus_table_names_no_mode_at_all() {
    // OFF IS THE DEFAULT, and it is the whole reason there is no `enabled`
    // key: a machine that never wrote the table behaves exactly as it did
    // before the table existed. MEASURED on this operator's own machine, a
    // Focus was asserted for 95% of one day, so a feature that shipped on
    // would have silenced almost everything pns raised that day.
    let config = parse_config("[plugins.hue]\nenabled = true\n").unwrap();
    assert!(config.focus_silence.is_empty());
}

#[test]
fn a_silence_list_that_is_not_a_list_is_refused_naming_the_key() {
    // `silence = "Sleep"` is what a hand writes first. Read as one name it
    // would work by accident; read as anything else it silences nothing
    // and says nothing, which is the state the operator cannot discover.
    let err = parse_config("[focus]\nsilence = \"Sleep\"\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("silence"),
                "the offender is named: {message}"
            );
            assert!(
                message.contains("focus"),
                "and so is the table it is in: {message}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_mode_name_that_is_not_a_string_is_refused_naming_the_key() {
    let err = parse_config("[focus]\nsilence = [\"Sleep\", 5]\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("silence"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_mode_name_that_is_the_empty_string_is_refused_by_name() {
    // AN ENTRY THAT NAMES NO MODE is a policy the operator believes they
    // wrote and pns can never act on, which is the misspelled key's own
    // failure one level down. `[recap] repos` refuses its empty entry for
    // this reason and this refusal is that rule, not a new one.
    let err = parse_config("[focus]\nsilence = [\"Sleep\", \"\"]\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("silence"),
                "the offender is named: {message}"
            );
            assert!(
                message.contains("focus"),
                "and so is the table it is in: {message}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn an_empty_silence_list_is_admitted_because_it_is_the_feature_switched_off() {
    // THE BOUNDARY OF THE REFUSAL ABOVE. An empty LIST is a working,
    // readable setting that says exactly what it does, so a refusal that
    // reached it would refuse the one config the template's own commented
    // block turns into when a mode is deleted from it.
    let config = parse_config("[focus]\nsilence = []\n").unwrap();
    assert!(config.focus_silence.is_empty());
}

#[test]
fn a_misspelled_focus_key_is_refused_by_name_rather_than_ignored() {
    // An unjudged key here is a Focus policy the operator believes they
    // wrote and pns never reads.
    let err = parse_config("[focus]\nsilenced = [\"Sleep\"]\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("silenced"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_non_table_focus_value_is_refused_naming_the_arm_rather_than_the_key() {
    // `recap = 5`'s sibling one table over, and asserted the same way: the
    // unknown-top-level-key refusal carries the word `focus` too, so an
    // assertion that asked only for the name would pass for the day the
    // admitting arm went missing entirely.
    let err = parse_config("focus = 5\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("focus"),
                "the offender is named: {message}"
            );
            assert!(
                message.contains("is not a table"),
                "and it is the non-table arm rather than the unknown-key one: {message}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}
