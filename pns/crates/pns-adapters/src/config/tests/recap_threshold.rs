use super::*;

#[test]
fn the_recaps_volume_threshold_is_a_count_the_operator_can_state() {
    // THE CALIBRATION KNOB. The locked threshold carries a tilde and has no
    // live measurement behind it, so it ships as a key defaulted to the
    // operator's own guess and the recap's header prints the real count
    // every time. One week of real recaps settles it without a rebuild.
    let config = parse_config("[recap]\nminimum_events = 3\n").unwrap();
    assert_eq!(config.recap.minimum_events, 3, "the stated count was read");
    assert!(
        config.recap.post_window_recap,
        "and the switches kept their defaults"
    );
    assert_eq!(
        parse_config("[plugins.lights]\nenabled = true\n")
            .unwrap()
            .recap
            .minimum_events,
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
    let err = parse_config("[recap]\nminimum_events = 0\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => {
            assert!(message.contains("minimum_events"), "{message}");
            assert!(
                message.contains('1'),
                "the refusal names the floor rather than only the offence: {message}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(
        parse_config("[recap]\nminimum_events = 1\n")
            .unwrap()
            .recap
            .minimum_events,
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
        let err = parse_config(&format!("[recap]\nminimum_events = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("minimum_events"),
                "the offender is named for {stated}: {message}"
            ),
            other => panic!("expected Invalid for {stated}, got {other:?}"),
        }
    }
}

#[test]
fn the_minimum_time_away_is_a_duration_the_operator_can_state() {
    let stated = |text: &str| parse_config(text).unwrap().recap.minimum_away;
    assert_eq!(
        stated("[recap]\nminimum_away = \"45m\"\n"),
        std::time::Duration::from_secs(45 * 60)
    );
    assert_eq!(
        stated("[plugins.lights]\nenabled = true\n"),
        std::time::Duration::from_secs(20 * 60),
        "an absent key is the shipped twenty minutes"
    );
    assert_eq!(
        stated("[recap]\nminimum_away = \"0s\"\n"),
        std::time::Duration::ZERO,
        "zero leaves the event count as the only bar"
    );
}

#[test]
fn a_minimum_time_away_that_is_not_a_duration_up_to_a_day_is_refused_naming_the_key() {
    for stated in ["20", "\"twenty\"", "\"25h\""] {
        let err = parse_config(&format!("[recap]\nminimum_away = {stated}\n")).unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("minimum_away"),
                "the offender is named for {stated}: {message}"
            ),
            other => panic!("expected Invalid for {stated}, got {other:?}"),
        }
    }
}
