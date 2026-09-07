use super::*;

#[test]
fn a_room_the_state_file_could_never_carry_is_refused_at_the_table() {
    // A ROOM IS THE BRIDGE'S OWN TEXT and crosses the state file verbatim,
    // so a name the reader would refuse renders no line at all. That is the
    // right answer for the WRITE, and a silent one for the operator: the
    // poll publishes nothing, the daemon re-arms at its normal interval and
    // ignores the exit status, and the doctor can only report a reading
    // that is stale or absent. Read here instead, it is a configuration
    // error with the room in it.
    for (shape, room) in [
        ("a tab", "3F\tStudio".to_string()),
        ("a newline", "3F\nStudio".to_string()),
        ("an empty name", String::new()),
        ("65 characters", "r".repeat(65)),
    ] {
        let said = match parse_presence(&presence_config(&format!(
            "type = \"hue\"\nrooms = [{}]\n",
            serde_json::to_string(&room).expect("a json string")
        ))) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("{shape}: a room the state file cannot carry was accepted"),
        };
        assert!(
            said.contains("rooms"),
            "{shape}: the refusal does not name the key: {said}"
        );
    }
    // AND THE BOUND ITSELF IS A ROOM, or the refusal is one character early
    // and nothing says so. Real names carry spaces, dashes and digits, and
    // a check that took any of those for malformed would silence the sensor
    // on the rooms it actually watches.
    let at_the_bound = "r".repeat(64);
    assert_eq!(
        parse_presence(&presence_config(&format!(
            "type = \"hue\"\nrooms = [\"3F - Studio\", \"{at_the_bound}\"]\n"
        )))
        .unwrap()
        .expect("the table is on")
        .rooms,
        vec!["3F - Studio".to_string(), at_the_bound]
    );
}

#[test]
fn a_presence_table_naming_no_backend_is_refused_by_name() {
    for body in ["", "type = \"aqara\"\n"] {
        let said = match parse_presence(&presence_config(body)) {
            Err(error) => error.detail().to_string(),
            Ok(_) => panic!("{body:?} names no backend this answers"),
        };
        assert!(
            said.contains("type") && said.contains("hue"),
            "the refusal names the key and the one backend: {said}"
        );
    }
}

#[test]
fn a_presence_table_the_operator_switched_off_is_inert_settings_and_all() {
    let config =
        parse_config("[plugins.presence]\nenabled = false\ntype = \"aqara\"\npoll_secs = 900\n")
            .unwrap();
    assert_eq!(parse_presence(&config).unwrap(), None);
}
