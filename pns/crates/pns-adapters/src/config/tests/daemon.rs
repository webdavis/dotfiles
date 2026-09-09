use super::*;

// --- [daemon] ------------------------------------------------------------

/// The clock's one switch, in the four states the table has.
///
/// DEFAULT ON, unlike `[focus]` and unlike a plugin, and the reason is that
/// this table gates nothing an operator can see. An idle daemon reads one
/// empty directory a second; default OFF would put both rider features
/// behind two switches, so enabling a light and seeing nothing would send
/// the operator hunting for a second, invisible one.
#[test]
fn the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name() {
    assert!(
        parse_config("").unwrap().daemon_enabled,
        "no table at all is the default, which is on"
    );
    assert!(
        parse_config("[daemon]\nenabled = true\n")
            .unwrap()
            .daemon_enabled
    );
    assert!(
        !parse_config("[daemon]\nenabled = false\n")
            .unwrap()
            .daemon_enabled
    );

    let err = parse_config("[daemon]\nenabled = \"yes\"\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("daemon") && message.contains("enabled"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }

    let err = parse_config("[daemon]\nenable = true\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("daemon") && message.contains("enable"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }

    let err = parse_config("daemon = 5\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("daemon") && message.contains("is not a table"),
            "and it is the non-table arm rather than the unknown-key one: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}
