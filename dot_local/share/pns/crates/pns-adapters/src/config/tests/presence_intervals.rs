use super::*;

#[test]
fn a_desk_that_speaks_for_no_time_at_all_is_refused_by_name() {
    // Zero is the desk override switched off by accident: no desk reading
    // could ever be fresher than a bound of zero seconds.
    let said = match parse_presence(&presence_config(
        "type = \"hue\"\ndesk_stale_after_secs = 0\n",
    )) {
        Err(error) => error.detail().to_string(),
        Ok(_) => panic!("a zero desk bound is refused"),
    };
    assert!(
        said.contains("desk_stale_after_secs") && said.contains('0'),
        "the refusal names the key and the value: {said}"
    );
}

#[test]
fn a_desk_bound_over_the_operational_maximum_is_refused_by_name() {
    // UNBOUNDED, THE KEY STOPS BEING A KNOB. A desk that never goes stale
    // wins every arbitration there is, so one mistyped digit parks the
    // lamps in `desk_room` for years with no reading able to move them and
    // nothing said about why.
    for stated in ["3601", "9223372036854775807"] {
        let said = match parse_presence(&presence_config(&format!(
            "type = \"hue\"\ndesk_stale_after_secs = {stated}\n"
        ))) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("`desk_stale_after_secs = {stated}` is over the maximum"),
        };
        assert!(
            said.contains("desk_stale_after_secs") && said.contains("3600"),
            "the refusal names the key and the bound: {said}"
        );
    }
}

#[test]
fn a_desk_bound_at_the_operational_maximum_is_accepted() {
    // THE BOUND IS INCLUSIVE, or it is one short and nobody could tell
    // from the outside.
    let config = presence_config("type = \"hue\"\ndesk_stale_after_secs = 3600\n");
    let presence = parse_presence(&config).unwrap().expect("the table is on");
    assert_eq!(presence.desk_stale_after_secs, 3600);
}

#[test]
fn a_presence_table_stating_no_intervals_takes_the_shipped_ones() {
    let config = presence_config("type = \"hue\"\n");
    let presence = parse_presence(&config).unwrap().expect("the table is on");
    assert_eq!(
        (
            presence.poll_secs,
            presence.stale_after_secs,
            presence.desk_stale_after_secs
        ),
        (5, 15, 120)
    );
}

#[test]
fn a_poll_interval_outside_its_range_is_refused_by_name_at_both_ends() {
    for stated in ["1", "61"] {
        let said = match parse_presence(&presence_config(&format!(
            "type = \"hue\"\npoll_secs = {stated}\n"
        ))) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("`poll_secs = {stated}` is outside the range"),
        };
        assert!(
            said.contains("poll_secs") && said.contains(stated),
            "the refusal names the key and the value: {said}"
        );
    }
    // AND THE ENDS THEMSELVES ARE INSIDE IT, or the bound is one short at
    // each end and nobody could tell from the refusal.
    for stated in ["2", "60"] {
        assert!(
            parse_presence(&presence_config(&format!(
                "type = \"hue\"\npoll_secs = {stated}\nstale_after_secs = 180\n"
            )))
            .is_ok(),
            "`poll_secs = {stated}` is inside the range"
        );
    }
}

#[test]
fn a_stale_bound_under_the_poll_interval_is_refused_by_name() {
    // Every reading would age out before the next poll could replace it,
    // so the sensor would answer unknown for good and never say why.
    let said = match parse_presence(&presence_config(
        "type = \"hue\"\npoll_secs = 30\nstale_after_secs = 20\n",
    )) {
        Err(error) => error.detail().to_string(),
        Ok(_) => panic!("a bound under the interval is refused"),
    };
    assert!(
        said.contains("stale_after_secs") && said.contains("poll_secs"),
        "the refusal names both keys: {said}"
    );
}
