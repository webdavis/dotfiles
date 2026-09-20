use super::*;

#[test]
fn a_desk_that_speaks_for_no_time_at_all_is_refused_by_name() {
    // Zero is the desk override switched off by accident: no desk reading
    // could ever be fresher than a bound of zero seconds.
    let said = match parse_presence(&presence_config(
        "type = \"hue\"\ndesk_input_max_age = \"0s\"\n",
    )) {
        Err(error) => error.detail().to_string(),
        Ok(_) => panic!("a zero desk bound is refused"),
    };
    assert!(
        said.contains("desk_input_max_age") && said.contains('0'),
        "the refusal names the key and the value: {said}"
    );
}

#[test]
fn a_desk_bound_over_the_operational_maximum_is_refused_by_name() {
    // UNBOUNDED, THE KEY STOPS BEING A KNOB. A desk that never goes stale
    // wins every arbitration there is, so one mistyped digit parks the
    // lamps in `desk_room` for years with no reading able to move them and
    // nothing said about why.
    for stated in ["3601s", "9223372036854775807h"] {
        let said = match parse_presence(&presence_config(&format!(
            "type = \"hue\"\ndesk_input_max_age = \"{stated}\"\n"
        ))) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("`desk_input_max_age = {stated:?}` is over the maximum"),
        };
        assert!(
            said.contains("desk_input_max_age") && said.contains("1h"),
            "the refusal names the key and the bound: {said}"
        );
    }
}

#[test]
fn a_desk_bound_at_the_operational_maximum_is_accepted() {
    // THE BOUND IS INCLUSIVE, or it is one short and nobody could tell
    // from the outside.
    let config = presence_config("type = \"hue\"\ndesk_input_max_age = \"1h\"\n");
    let presence = parse_presence(&config).unwrap().expect("the table is on");
    assert_eq!(presence.desk_input_max_age_secs, 3600);
}

#[test]
fn a_presence_table_stating_no_durations_takes_the_shipped_ones() {
    let config = presence_config("type = \"hue\"\n");
    let presence = parse_presence(&config).unwrap().expect("the table is on");
    assert_eq!(
        (
            presence.poll_interval_secs,
            presence.reading_max_age_secs,
            presence.desk_input_max_age_secs
        ),
        (5, 15, 120)
    );
}

#[test]
fn the_three_durations_reach_the_settings_the_seconds_used_to() {
    // EACH IS A DURATION STRING NOW, read through the one parser every
    // other duration in the file goes through, and landing on the same
    // whole seconds the reader has always acted on.
    let config = presence_config(
        "type = \"hue\"\npoll_interval = \"10s\"\nreading_max_age = \"1m\"\n\
         desk_input_max_age = \"5m\"\n",
    );
    let presence = parse_presence(&config).unwrap().expect("the table is on");
    assert_eq!(
        (
            presence.poll_interval_secs,
            presence.reading_max_age_secs,
            presence.desk_input_max_age_secs
        ),
        (10, 60, 300)
    );
}

#[test]
fn a_poll_interval_outside_its_range_is_refused_by_name_at_both_ends() {
    for stated in ["1s", "61s"] {
        let said = match parse_presence(&presence_config(&format!(
            "type = \"hue\"\npoll_interval = \"{stated}\"\n"
        ))) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("`poll_interval = {stated:?}` is outside the range"),
        };
        assert!(
            said.contains("poll_interval") && said.contains(stated),
            "the refusal names the key and the value: {said}"
        );
    }
    // AND THE ENDS THEMSELVES ARE INSIDE IT, or the bound is one short at
    // each end and nobody could tell from the refusal.
    for stated in ["2s", "1m"] {
        assert!(
            parse_presence(&presence_config(&format!(
                "type = \"hue\"\npoll_interval = \"{stated}\"\nreading_max_age = \"3m\"\n"
            )))
            .is_ok(),
            "`poll_interval = {stated:?}` is inside the range"
        );
    }
}

#[test]
fn a_reading_bound_under_the_poll_interval_is_refused_by_name() {
    // Every reading would age out before the next poll could replace it,
    // so the sensor would answer unknown for good and never say why.
    let said = match parse_presence(&presence_config(
        "type = \"hue\"\npoll_interval = \"30s\"\nreading_max_age = \"20s\"\n",
    )) {
        Err(error) => error.detail().to_string(),
        Ok(_) => panic!("a bound under the interval is refused"),
    };
    assert!(
        said.contains("reading_max_age") && said.contains("poll_interval"),
        "the refusal names both keys: {said}"
    );
}

#[test]
fn a_duration_key_written_as_a_bare_count_is_refused_by_name() {
    // The old spelling's VALUE is the other half of the rename: a bare
    // number meant seconds here and minutes elsewhere, which is what the
    // duration vocabulary exists to end.
    for key in ["poll_interval", "reading_max_age", "desk_input_max_age"] {
        let said = match parse_presence(&presence_config(&format!("type = \"hue\"\n{key} = 5\n"))) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("`{key} = 5` is not a duration"),
        };
        assert!(
            said.contains(key) && said.contains("duration"),
            "the refusal names the key and asks for a duration: {said}"
        );
    }
}

#[test]
fn every_old_presence_key_is_refused_by_name_with_the_roster_beside_it() {
    // A FILE THAT MISSED THE RENAME IS REFUSED, not half-read: each of
    // these parsed and did something yesterday, so accepting the table
    // without them would leave the sensor polling at a default the
    // operator believes they changed.
    for (retired, replacement) in [
        ("exclude = []", "excluded_rooms"),
        ("poll_secs = 5", "poll_interval"),
        ("stale_after_secs = 15", "reading_max_age"),
        ("desk_stale_after_secs = 120", "desk_input_max_age"),
    ] {
        let said = match parse_config(&format!(
            "[plugins.lights]\nenabled = true\n\
             [plugins.presence]\nenabled = true\ntype = \"hue\"\n{retired}\n"
        )) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("the retired `{retired}` was accepted"),
        };
        let key = retired.split(' ').next().expect("a key");
        assert!(
            said.contains(key) && said.contains(replacement),
            "the refusal names the retired key and its replacement: {said}"
        );
    }
}
