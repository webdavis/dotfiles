use super::*;

// --- the recap's switches -----------------------------------------------

#[test]
fn a_recap_table_is_read_rather_than_refused_and_each_switch_stands_alone() {
    // ONE KEY STATED, THE OTHER TWO UNTOUCHED. The three deliveries are
    // independent, so an operator who silenced the recap must not find
    // they also silenced the catch-up card, or the other way round.
    let config = parse_config("[recap]\ndigest = false\n").unwrap();
    assert!(!config.recap.digest, "the stated switch was read");
    assert!(config.recap.replay_card, "the card kept its default");
    assert!(config.recap.digest_as_thread, "the thread kept its default");
}

#[test]
fn a_config_with_no_recap_table_leaves_every_switch_on() {
    // ABSENT IS ALL ON, which is what makes the table optional: a machine
    // that never writes one behaves exactly as it did before the table
    // existed. The direction is STATED rather than derived, because a
    // derived default is all-off, and that would silently take the
    // catch-up card away from every machine whose config predates this.
    let config = parse_config("[plugins.hue]\nenabled = true\n").unwrap();
    assert!(config.recap.replay_card, "the catch-up card");
    assert!(config.recap.digest, "the recap");
    assert!(config.recap.digest_as_thread, "the recap's own thread");
}

#[test]
fn a_misspelled_recap_key_is_refused_by_name_rather_than_left_at_its_default() {
    // UNKNOWN KEYS REFUSE HERE, unlike a plugin's free-form settings, and
    // the difference is who reads them: a plugin table is handed to a
    // plugin this layer cannot judge, while this table is read here and
    // nowhere else. An unjudged key is a typo that leaves the switch ON
    // while the operator believes they turned it off.
    let err = parse_config("[recap]\nreplaycard = false\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("replaycard"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_non_boolean_recap_switch_is_refused_naming_the_key() {
    // `digest = "yes"` read as a switch is the same defect one level down
    // from a non-boolean `enabled`: the operator asked for something, did
    // not get it, and was told nothing.
    let err = parse_config("[recap]\ndigest = \"yes\"\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("digest"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_top_level_key_that_merely_looks_like_recap_is_still_refused_by_name() {
    // GUARD. Admitting `[recap]` admits ONE more key and nothing else:
    // the plural typo, newly plausible now that the singular parses, has
    // to name itself rather than sit there as a table nothing reads. The
    // retired `[home]` table's test guards the same arm from the other
    // side, and both must stay green as the arm grows.
    let err = parse_config("[recaps]\ndigest = false\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("recaps"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_non_table_recap_value_is_refused_naming_the_key() {
    // `recap = 5` is `plugins = 5` one table over: a value at the key the
    // switches hang off must refuse, never fall through to the all-on
    // default and leave the operator believing their file was read.
    //
    // THE ARM IS NAMED, not just the key. "unknown top-level key `recap`"
    // is what comes back when the admitting arm is gone entirely, and it
    // carries the word `recap` too: an assertion that asked only for the
    // name would pass for the refusal that says the table is not a
    // setting at all, which is a different fault with a different fix.
    let err = parse_config("recap = 5\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(
                message.contains("recap"),
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
