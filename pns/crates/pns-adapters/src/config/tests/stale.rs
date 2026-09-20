use super::*;

// --- [stale] ----------------------------------------------------------------

/// The escalation's window, on `[remind] delay`'s terms: the switch and the
/// timing in one value.
///
/// DEFAULT AN HOUR AND NOT OFF, which is where it differs from the reminder.
/// The nudge INTERRUPTS a session the operator is already looking at, so it
/// waits to be asked for; an hour-old block is a session nobody is coming back
/// to, and the default that does nothing is the one that lets it sit there.
#[test]
fn the_escalation_window_defaults_to_an_hour_and_the_switch_is_what_turns_it_off() {
    assert_eq!(
        parse_config("[stale]\n").unwrap().stale_escalate_after_secs,
        3600,
        "a table with nothing said carries the default window"
    );
    assert!(
        parse_config("[stale]\n").unwrap().stale_enabled,
        "and the switch over it defaults on"
    );
    let off = parse_config("[stale]\nenabled = false\n").unwrap();
    assert_eq!(
        off.stale_escalate_after_secs, 3600,
        "the window is kept whole while the switch is off"
    );
    assert_eq!(
        off.stale_window_secs(),
        0,
        "and the window read by the page is zero, which is the feature off"
    );
    // ZERO IS REFUSED BY NAME, because the window is not the switch: with
    // an unset window meaning an hour there is nothing absence could say.
    let said = refusal("[stale]\nescalate_after = \"0s\"\n");
    assert!(
        said.contains("stale") && said.contains("escalate_after") && said.contains("unset"),
        "the refusal names the table, the key and the off statement: {said}"
    );
    assert_eq!(
        parse_config("[stale]\nescalate_after = \"1m\"\n")
            .unwrap()
            .stale_escalate_after_secs,
        60
    );
    assert_eq!(
        parse_config("[stale]\nescalate_after = \"24h\"\n")
            .unwrap()
            .stale_escalate_after_secs,
        86_400
    );
    assert_eq!(
        parse_config("").unwrap().stale_escalate_after_secs,
        3600,
        "a file with no table at all still escalates, because the window is \
         the default rather than the table's presence"
    );
}

#[test]
fn an_escalation_window_that_is_not_a_duration_is_refused_by_name() {
    for (case, text) in [
        ("a bare number", "[stale]\nescalate_after = 3600\n"),
        ("negative", "[stale]\nescalate_after = \"-1m\"\n"),
        ("no unit", "[stale]\nescalate_after = \"3600\"\n"),
        ("under the floor", "[stale]\nescalate_after = \"59s\"\n"),
        ("over the ceiling", "[stale]\nescalate_after = \"25h\"\n"),
    ] {
        match parse_config(text).unwrap_err() {
            ConfigError::Invalid(message) => assert!(
                message.contains("stale") && message.contains("escalate_after"),
                "{case}: the offender is named: {message}"
            ),
            other => panic!("{case}: expected Invalid, got {other:?}"),
        }
    }
}

/// The route the page takes, which is unset until the operator names one.
///
/// UNSET IS NOT A SECOND SPELLING OF THE URGENT ROUTE. The page carries the
/// health kind, and the event path resolves that against `[routes]`, so a
/// name here is an override rather than a restatement.
#[test]
fn the_page_route_is_unset_until_it_is_named_and_an_unusable_name_is_refused() {
    assert_eq!(parse_config("[stale]\n").unwrap().stale_route, None);
    assert_eq!(
        parse_config("[stale]\nroute = \"priority\"\n")
            .unwrap()
            .stale_route,
        Some("priority".to_string())
    );
    for (case, text) in [
        ("a path separator", "[stale]\nroute = \"a/b\"\n"),
        ("empty", "[stale]\nroute = \"\"\n"),
        ("not a string", "[stale]\nroute = 3\n"),
    ] {
        match parse_config(text).unwrap_err() {
            ConfigError::Invalid(message) => assert!(
                message.contains("stale") && message.contains("route"),
                "{case}: the offender is named: {message}"
            ),
            other => panic!("{case}: expected Invalid, got {other:?}"),
        }
    }
}
