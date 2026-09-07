use super::*;

#[test]
fn an_armed_presence_table_reads_its_rooms_its_exclusions_and_its_two_intervals() {
    let config = presence_config(
        "type = \"hue\"\nrooms = [\"3F - Studio\", \"2F - Kitchen\"]\n\
             exclude = [\"3F - MBedroom\"]\npoll_secs = 10\nstale_after_secs = 30\n",
    );
    assert_eq!(
        parse_presence(&config).unwrap(),
        Some(Presence {
            rooms: vec!["3F - Studio".to_string(), "2F - Kitchen".to_string()],
            exclude: vec!["3F - MBedroom".to_string()],
            desk_room: None,
            desk_stale_after_secs: 120,
            poll_secs: 10,
            stale_after_secs: 30,
        })
    );
}

#[test]
fn an_armed_presence_table_reads_the_room_its_desk_is_in() {
    let config = presence_config(
        "type = \"hue\"\nrooms = [\"3F - Studio\", \"2F - Kitchen\"]\n\
             desk_room = \"3F - Studio\"\n",
    );
    assert_eq!(
        parse_presence(&config).unwrap().unwrap().desk_room,
        Some("3F - Studio".to_string())
    );
}

#[test]
fn a_desk_room_the_watch_list_does_not_name_is_refused_by_name() {
    // A desk in a room no reading can ever name would narrow the lamps to
    // a room the poll never watches, silently, forever.
    let said = match parse_presence(&presence_config(
        "type = \"hue\"\nrooms = [\"2F - Kitchen\"]\ndesk_room = \"3F - Studio\"\n",
    )) {
        Err(error) => error.detail().to_string(),
        Ok(_) => panic!("a desk room outside `rooms` is refused"),
    };
    assert!(
        said.contains("desk_room") && said.contains("3F - Studio") && said.contains("rooms"),
        "the refusal names the key, the value and the list it is not in: {said}"
    );
}

#[test]
fn a_desk_room_the_operator_also_excluded_is_refused_by_name() {
    // The two keys contradict each other: `exclude` says never light that
    // room and `desk_room` says light it whenever the desk is warm. Loaded,
    // the desk branch wins and the exclusion is silently a lie.
    let said = match parse_presence(&presence_config(
        "type = \"hue\"\nrooms = [\"3F - Studio\"]\nexclude = [\"3F - Studio\"]\n\
             desk_room = \"3F - Studio\"\n",
    )) {
        Err(error) => error.detail().to_string(),
        Ok(_) => panic!("a desk room the config also excludes is refused"),
    };
    assert!(
        said.contains("desk_room") && said.contains("exclude") && said.contains("3F - Studio"),
        "the refusal names both keys and the value: {said}"
    );
}

#[test]
fn an_empty_room_name_is_refused_wherever_it_is_written() {
    // An empty entry matches no bridge room, and in `rooms` it also lets an
    // empty `desk_room` past the membership check into the narrowing.
    //
    // `rooms` IS NOT HERE because `room_fits` refuses it first and in its
    // own words, pinned by
    // `a_room_the_state_file_could_never_carry_is_refused_at_the_table`.
    for (key, body) in [
        (
            "exclude",
            "type = \"hue\"\nrooms = [\"3F - Studio\"]\nexclude = [\"\"]\n",
        ),
        (
            "desk_room",
            "type = \"hue\"\nrooms = [\"3F - Studio\"]\ndesk_room = \"\"\n",
        ),
    ] {
        let said = match parse_presence(&presence_config(body)) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("an empty `{key}` entry is refused"),
        };
        assert!(
            said.contains(key) && said.contains("empty"),
            "the refusal names `{key}` and says it is empty: {said}"
        );
    }
}
