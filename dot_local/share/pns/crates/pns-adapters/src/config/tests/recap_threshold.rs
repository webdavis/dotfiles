use super::*;

#[test]
fn the_recaps_volume_threshold_is_a_count_the_operator_can_state() {
    // THE CALIBRATION KNOB. The locked threshold carries a tilde and has no
    // live measurement behind it, so it ships as a key defaulted to the
    // operator's own guess and the recap's header prints the real count
    // every time. One week of real recaps settles it without a rebuild.
    let config = parse_config("[recap]\nmin_events = 3\n").unwrap();
    assert_eq!(config.recap.min_events, 3, "the stated count was read");
    assert!(config.recap.digest, "and the switches kept their defaults");
    assert_eq!(
        parse_config("[plugins.hue]\nenabled = true\n")
            .unwrap()
            .recap
            .min_events,
        8,
        "an absent key is the operator's stated eight"
    );
}

#[test]
fn a_volume_threshold_of_zero_is_refused_by_name_rather_than_read_as_every_event() {
    // ZERO IS NOT A THRESHOLD. `counted.len() >= 0` is always true, so it
    // recaps every single event, including one over an EMPTY window, which
    // is the one state the recap body says the event path never posts. An
    // operator calibrating the knob downward would get a card and a Discord
    // recap on every event, each saying nothing was recorded.
    let err = parse_config("[recap]\nmin_events = 0\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(message.contains("min_events"), "{message}");
            assert!(
                message.contains('1'),
                "the refusal names the floor rather than only the offence: {message}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(
        parse_config("[recap]\nmin_events = 1\n")
            .unwrap()
            .recap
            .min_events,
        1,
        "and one is accepted: it means any activity at all"
    );
}

#[test]
fn a_volume_threshold_that_is_not_a_count_is_refused_naming_the_key() {
    // A STRING, A FRACTION AND A NEGATIVE are each a threshold the operator
    // asked for and would not get, and each has to say so rather than leave
    // the count silently at its default.
    for stated in ["\"eight\"", "8.5", "-1", "true"] {
        let err = parse_config(&format!("[recap]\nmin_events = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("min_events"),
                "the offender is named for {stated}: {message}"
            ),
            other => panic!("expected Invalid for {stated}, got {other:?}"),
        }
    }
}
