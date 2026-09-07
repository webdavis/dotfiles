use super::*;

// --- reading the assertion store ----------------------------------------

#[test]
fn a_store_holding_a_live_assertion_names_the_mode_that_is_on() {
    assert_eq!(
        active_modes(LIVE_ACTIVE),
        asserted(&[CASUALLY_CONCERNED]),
        "the live record's own mode identifier is the answer"
    );
}

#[test]
fn the_same_record_written_twice_is_one_mode_and_not_two() {
    // THE LIVE FILE REALLY DOES THIS: two records carry one
    // `assertionUUID`. Uniqueness is not a property macOS maintains, so a
    // reader that counted these would be counting its own duplicate.
    assert_eq!(active_modes(LIVE_ACTIVE).len(), 1);
}

#[test]
fn every_mode_asserted_at_once_is_named_and_not_just_the_first() {
    // A reader that assumed one live Focus, or that took `[0]` because the
    // records "should" be unique, would silence on one of these and miss
    // the other. The operator's own history really overlaps two.
    assert_eq!(
        active_modes(TWO_AT_ONCE),
        asserted(&[CASUALLY_CONCERNED, "com.apple.sleep.sleep-mode"])
    );
    for listed in ["Casually Concerned", "Sleep"] {
        assert!(
            silenced(
                &active_modes(TWO_AT_ONCE),
                &mode_names(LIVE_CATALOG),
                &named(&[listed])
            ),
            "naming either one is enough: {listed}"
        );
    }
}

#[test]
fn both_documented_spellings_of_no_focus_name_no_mode() {
    // The two third-party descriptions of the empty state DISAGREE with
    // each other, so both are pinned. This is also the pair that catches a
    // substring grep for `storeAssertionRecords`, which is the
    // implementation shipped elsewhere and is right about only one of them.
    assert!(
        active_modes(EMPTIED_ARRAY).is_empty(),
        "an emptied array is no Focus"
    );
    assert!(
        active_modes(KEY_ABSENT).is_empty(),
        "an absent key is no Focus"
    );
}

#[test]
fn an_ended_focus_in_the_invalidation_history_is_never_an_active_one() {
    // Deactivation MOVES the record rather than deleting it, so both empty
    // fixtures still carry a mode identifier further down the file. A
    // reader pointed at the wrong array reports a Focus that ended weeks
    // ago as the one that is on now.
    assert!(
        !active_modes(EMPTIED_ARRAY).contains("com.apple.donotdisturb.mode.workout"),
        "the workout Focus in the history had already ended"
    );
    assert!(
        !active_modes(KEY_ABSENT).contains(CASUALLY_CONCERNED),
        "and so had this one"
    );
}

#[test]
fn nothing_readable_names_no_mode_one_row_per_failure_shape() {
    for (shape, why) in [
        ("", "an empty file"),
        ("{", "a truncated document"),
        ("null", "a JSON null"),
        ("[]", "a top level array"),
        (r#"{"data":[]}"#, "no store in the data"),
        (r#"{"data":[{}]}"#, "a store with no arrays"),
        (
            r#"{"data":[{"storeAssertionRecords":{}}]}"#,
            "the records as an object",
        ),
        (
            r#"{"data":[{"storeAssertionRecords":["not a record"]}]}"#,
            "a record that is not an object",
        ),
        (
            r#"{"data":[{"storeAssertionRecords":[{"assertionDetails":{}}]}]}"#,
            "a record naming no mode",
        ),
        (
            // A JSON STREAM, restated for serde: `from_str` refuses
            // trailing content, and this row is what proves the refusal
            // lands on the SAFE side rather than reading the first
            // document and ignoring the rest.
            r#"{"data":[{"storeAssertionRecords":[]}]} {"data":[{"storeAssertionRecords":[{"assertionDetails":{"assertionDetailsModeIdentifier":"x"}}]}]}"#,
            "two concatenated documents",
        ),
    ] {
        assert!(
            active_modes(shape).is_empty(),
            "{why} is not a Focus: {shape}"
        );
    }
}
