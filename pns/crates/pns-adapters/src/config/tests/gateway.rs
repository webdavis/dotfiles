use super::*;

// --- [gateway] ------------------------------------------------------------

/// The clock's one switch, in the four states the table has.
///
/// DEFAULT ON, unlike `[focus]` and unlike a plugin, and the reason is that
/// this table gates nothing an operator can see. An idle daemon reads one
/// empty directory a second; default OFF would put both rider features
/// behind two switches, so enabling a light and seeing nothing would send
/// the operator hunting for a second, invisible one.
#[test]
fn the_gateway_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name() {
    assert!(
        parse_config("").unwrap().gateway_enabled,
        "no table at all is the default, which is on"
    );
    assert!(
        parse_config("[gateway]\nenabled = true\n")
            .unwrap()
            .gateway_enabled
    );
    assert!(
        !parse_config("[gateway]\nenabled = false\n")
            .unwrap()
            .gateway_enabled
    );

    let err = parse_config("[gateway]\nenabled = \"yes\"\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("gateway") && message.contains("enabled"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }

    let err = parse_config("[gateway]\nenable = true\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("gateway") && message.contains("enable"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }

    let err = parse_config("gateway = 5\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("gateway") && message.contains("is not a table"),
            "and it is the non-table arm rather than the unknown-key one: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

/// `[gateway] service`: the launchd label `pns gateway` acts on. NO DEFAULT,
/// unlike `enabled` beside it: an absent key is `None`, which is what every
/// gateway verb refuses on rather than a label pns guessed at.
#[test]
fn the_gateway_service_key_has_no_default_and_refuses_the_wrong_type_by_name() {
    assert_eq!(
        parse_config("").unwrap().gateway_service,
        None,
        "no table at all names no service"
    );
    assert_eq!(
        parse_config("[gateway]\nenabled = true\n")
            .unwrap()
            .gateway_service,
        None,
        "a table that never mentions it still names none"
    );
    assert_eq!(
        parse_config("[gateway]\nservice = \"com.example.pns-daemon\"\n")
            .unwrap()
            .gateway_service
            .as_deref(),
        Some("com.example.pns-daemon")
    );
    // BOTH KEYS TOGETHER, so one setting reading right does not depend on
    // the other being absent.
    assert_eq!(
        parse_config("[gateway]\nenabled = false\nservice = \"com.example.pns-daemon\"\n")
            .unwrap()
            .gateway_service
            .as_deref(),
        Some("com.example.pns-daemon")
    );

    let err = parse_config("[gateway]\nservice = 5\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("gateway") && message.contains("service"),
            "the offender is named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

/// The old heading, refused by name with the new one in the sentence.
///
/// NOT AMONG THE UNKNOWN TABLES: a file still holding `[daemon]` is refused
/// whole, which takes every plugin's secret with it, so the operator is told
/// the heading to write rather than handed the top-level vocabulary.
#[test]
fn the_old_daemon_heading_is_refused_naming_the_gateway_one() {
    let err = parse_config("[daemon]\nenabled = false\n").unwrap_err();
    match err {
        ConfigError::Invalid(message) => assert!(
            message.contains("`[daemon]`") && message.contains("`[gateway]`"),
            "both headings are named: {message}"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}
