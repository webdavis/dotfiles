//! The home probe, pinned: settings.

use super::fixtures::*;

// --- the settings --------------------------------------------------------

#[test]
fn no_router_plugin_table_at_all_is_not_configured_naming_the_table() {
    // hermes rides along so this is a MISS on the router's own name and
    // not a config the parser dropped whole.
    let config = crate::config::parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
    assert_eq!(
        enabled_router_table(&config),
        Err(SetupFailure::NoRouterPlugin)
    );
    let line = setup_report(&SetupFailure::NoRouterPlugin);
    assert!(line.contains("not configured"), "got: {line}");
    assert!(line.contains("[plugins.router]"), "got: {line}");
}

#[test]
fn a_router_table_switched_off_is_told_apart_from_no_table_at_all() {
    // Selection is the operator's, and a probe they turned off must not
    // read as one they never wrote: the first is fixed by flipping a flag
    // they are looking at, the second by writing a table.
    let config = crate::config::parse_config(
        "[plugins.router]\nenabled = false\ntype = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n",
    )
    .unwrap();
    assert_eq!(
        enabled_router_table(&config),
        Err(SetupFailure::RouterDisabled)
    );
    let disabled = setup_report(&SetupFailure::RouterDisabled);
    assert_ne!(disabled, setup_report(&SetupFailure::NoRouterPlugin));
    assert!(disabled.contains("[plugins.router]"), "got: {disabled}");
    assert!(disabled.contains("enabled = false"), "got: {disabled}");
}

#[test]
fn a_router_table_with_no_type_names_the_key_and_the_one_type_that_answers() {
    // WHICH ROUTER answers is the first question the table has to settle,
    // because every setting under it belongs to that one. A non-string
    // type and an EMPTY one are the same hole: there is no name to match
    // a backend by, and the empty one used to be quoted back as a type
    // nothing implements, which points at a value the operator never
    // typed instead of at the key they left blank.
    for text in [
        "router_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
        "type = 5\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
        "type = \"\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
    ] {
        assert_eq!(
            router_settings(&table(text)),
            Err(SetupFailure::NoType),
            "case: {text:?}"
        );
    }
    let line = setup_report(&SetupFailure::NoType);
    assert!(line.contains("type"), "got: {line}");
    assert!(line.contains("[plugins.router]"), "got: {line}");
    assert!(line.contains("\"unifi\""), "got: {line}");
}

#[test]
fn a_type_no_compiled_in_backend_answers_is_refused_quoting_it() {
    // Silently probing a UniFi endpoint on a router that is not one would
    // read Unknown forever with nothing to look at; the refusal quotes
    // what was asked for and says what this binary can answer.
    let asus = table("type = \"asus\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n");
    assert_eq!(
        router_settings(&asus),
        Err(SetupFailure::UnknownType("asus".to_string()))
    );
    let line = setup_report(&SetupFailure::UnknownType("asus".to_string()));
    assert!(line.contains("\"asus\""), "got: {line}");
    assert!(line.contains("\"unifi\""), "got: {line}");
    assert!(line.contains("[plugins.router]"), "got: {line}");
}

#[test]
fn a_missing_empty_or_mistyped_url_reports_the_invalid_table_line() {
    // A present-but-wrong VALUE is fixed by editing one line; a missing
    // TABLE is fixed by writing one. `router_url = 5` reported as "no
    // table" used to send the operator to write a table they already had.
    let named = "type = \"unifi\"\n";
    for text in [
        "device_hostname = \"mister\"\n",
        "router_url = \"\"\ndevice_hostname = \"mister\"\n",
        "router_url = 5\ndevice_hostname = \"mister\"\n",
    ] {
        assert_eq!(
            router_settings(&table(&format!("{named}{text}"))),
            Err(SetupFailure::InvalidRouterTable),
            "case: {text:?}"
        );
    }
    let invalid = setup_report(&SetupFailure::InvalidRouterTable);
    assert_ne!(invalid, setup_report(&SetupFailure::NoRouterPlugin));
    assert!(invalid.contains("[plugins.router]"), "got: {invalid}");
    assert!(invalid.contains("router_url"), "got: {invalid}");
    // The line stops naming the device keys: each of the three has its
    // own refusal now, and one covering all four sends the operator to
    // read four keys to find the one that is wrong.
    assert!(!invalid.contains("device_"), "got: {invalid}");
}

// --- the device identity -------------------------------------------------

#[test]
fn a_router_table_naming_no_device_at_all_is_refused_naming_every_key() {
    // Absent is not configured, and all three absent is a probe with
    // nothing to look for. The line has to spell all three keys: "no
    // device identifier" on its own sends the operator to the docs to
    // find out what one is called.
    assert_eq!(
        device_identity(&table(
            "type = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\n"
        )),
        Err(SetupFailure::NoDeviceIdentifier)
    );
    let line = setup_report(&SetupFailure::NoDeviceIdentifier);
    assert!(line.contains("device_mac"), "got: {line}");
    assert!(line.contains("device_hostname"), "got: {line}");
    assert!(line.contains("device_ipv4"), "got: {line}");
}

#[test]
fn a_table_carrying_only_a_hostname_yields_it_and_the_retired_key_is_not_read() {
    assert_eq!(
        device_identity(&table("device_hostname = \"mister\"\n")),
        Ok(DeviceIdentity::new(Some("mister".to_string()), None, None).unwrap())
    );
    // `phone` is the RETIRED spelling. Reading it as an identifier would
    // hide the rename the operator still has to make, and this repo does
    // not carry compatibility for its own unshipped code.
    assert_eq!(
        device_identity(&table("phone = \"mister\"\n")),
        Err(SetupFailure::NoDeviceIdentifier)
    );
    // Present but EMPTY, and present but the wrong TYPE, are the same
    // hole: the key is there and there is no device in it. Read as
    // absent, both would report "no device to look for" while the
    // operator is looking straight at a value they typed.
    // The wrong type is quoted back BY TYPE, because an integer has no
    // spelling worth echoing and the type is what has to change.
    for (text, found) in [
        ("device_hostname = \"\"\n", "\"\""),
        ("device_hostname = 5\n", "<integer>"),
        ("device_hostname = [\"mister\"]\n", "<array>"),
    ] {
        assert_eq!(
            device_identity(&table(text)),
            Err(SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Hostname,
                found: found.to_string(),
            }),
            "case: {text:?}"
        );
    }
    let line = setup_report(&SetupFailure::InvalidDeviceKey {
        key: DeviceKey::Hostname,
        found: "<integer>".to_string(),
    });
    assert!(line.contains("device_hostname"), "got: {line}");
    assert!(line.contains("<integer>"), "got: {line}");
}

#[test]
fn a_malformed_device_ipv4_is_refused_naming_the_key_and_quoting_the_value() {
    // The stdlib parser IS the validator, and these are the shapes it
    // refuses, measured on this toolchain (rustc 1.92.0-nightly): a
    // leading zero, a short quad, a long one, an octet past 255, and
    // surrounding whitespace. A hand-rolled octet parser would have to
    // relearn every one of them.
    for bad in [
        "010.1.1.1",
        "1.2.3",
        "1.2.3.4.5",
        "256.1.1.1",
        " 1.2.3.4",
        "1.2.3.4 ",
        "",
    ] {
        assert_eq!(
            device_identity(&table(&format!("device_ipv4 = \"{bad}\"\n"))),
            Err(SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Ipv4,
                found: format!("{bad:?}"),
            }),
            "case: {bad:?}"
        );
    }
    // The line quotes what was typed, so the typo is visible without
    // opening the file.
    let line = setup_report(&SetupFailure::InvalidDeviceKey {
        key: DeviceKey::Ipv4,
        found: "\"1.2.3\"".to_string(),
    });
    assert!(line.contains("device_ipv4"), "got: {line}");
    assert!(line.contains("\"1.2.3\""), "got: {line}");
    // A well-formed one is kept as an ADDRESS, never as the text it was
    // typed as: nothing downstream can compare it as a string.
    assert_eq!(
        device_identity(&table("device_ipv4 = \"192.168.1.169\"\n")),
        Ok(
            DeviceIdentity::new(None, Some(std::net::Ipv4Addr::new(192, 168, 1, 169)), None)
                .unwrap()
        )
    );
}

#[test]
fn a_malformed_device_mac_is_refused_naming_the_key_and_quoting_the_value() {
    // Too few groups, too many, a bare 12-hex run, a non-hex digit, a
    // group that is not exactly two digits, a MIXED separator, and a
    // trailing one. A single uniform separator is what tells a MAC from
    // a typo, and accepting the bare run would mean guessing at
    // groupings the router never uses.
    for bad in [
        "2e:11:ab:6d:b0",
        "2e:11:ab:6d:b0:4f:aa",
        "2e11ab6db04f",
        "zz:11:ab:6d:b0:4f",
        "2e:1:ab:6d:b0:4f",
        "2e-11:ab-6d:b0-4f",
        "2e:11:ab:6d:b0:4f:",
    ] {
        assert_eq!(
            device_identity(&table(&format!("device_mac = \"{bad}\"\n"))),
            Err(SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Mac,
                found: format!("{bad:?}"),
            }),
            "case: {bad:?}"
        );
    }
    let line = setup_report(&SetupFailure::InvalidDeviceKey {
        key: DeviceKey::Mac,
        found: "\"2e11ab6db04f\"".to_string(),
    });
    assert!(line.contains("device_mac"), "got: {line}");
    assert!(line.contains("\"2e11ab6db04f\""), "got: {line}");
}

#[test]
fn every_way_the_router_table_fails_to_provide_a_key_is_quietly_not_set_up() {
    for router in ["", "api_key = \"\"\n", "api_key = 5\n"] {
        assert_eq!(router_api_key(&table(router)), None, "case: {router:?}");
    }
    // And the line sends the operator to the table the key now lives in.
    let line = setup_report(&SetupFailure::NoApiKey);
    assert!(line.contains("[plugins.router]"), "got: {line}");
    assert!(line.contains("api_key"), "got: {line}");
}
