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
fn the_router_names_its_backend_with_type_and_no_longer_with_brand() {
    // ONE WORD ACROSS THE FILE. `type` is what selects a backend under
    // every table that has one to select, so the router's old `brand` is
    // not a second spelling of it: it names no backend, and a table
    // carrying it is refused exactly as an empty one is.
    let typed = table(
        "type = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n",
    );
    assert!(router_settings(&typed).is_ok(), "`type` names the backend");
    let branded =
        table("brand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n");
    assert!(
        router_settings(&branded).is_err(),
        "`brand` no longer names one"
    );
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
fn an_enabled_unifi_router_table_yields_its_url_and_device() {
    // The whole value path in one: the config's `[plugins.router]` table,
    // through the selection gate, into the two settings the probe runs on.
    let config = crate::config::parse_config(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
    )
    .unwrap();
    let router = enabled_router_table(&config).expect("the enabled table");
    assert_eq!(
        router_settings(router),
        Ok(RouterSettings {
            router_url: "https://192.168.1.1".to_string(),
            device: identity("device_hostname = \"mister\"\n"),
        })
    );
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
        Ok(DeviceIdentity {
            hostname: Some("mister".to_string()),
            ipv4: None,
            mac: None,
        })
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
        Ok(DeviceIdentity {
            hostname: None,
            ipv4: Some(std::net::Ipv4Addr::new(192, 168, 1, 169)),
            mac: None,
        })
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
fn a_well_formed_mac_in_any_case_or_separator_validates_to_one_spelling() {
    // ONE spelling is what makes the two sides comparable at all: the
    // operator may copy the MAC off a sticker in uppercase with dashes,
    // and the UDR answers in lowercase with colons.
    for typed in [
        "2e:11:ab:6d:b0:4f",
        "2E:11:AB:6D:B0:4F",
        "2e-11-ab-6d-b0-4f",
        "2E-11-AB-6D-B0-4F",
    ] {
        assert_eq!(
            device_identity(&table(&format!("device_mac = \"{typed}\"\n"))),
            Ok(DeviceIdentity {
                hostname: None,
                ipv4: None,
                mac: Some("2e:11:ab:6d:b0:4f".to_string()),
            }),
            "case: {typed:?}"
        );
    }
}

// --- the secret ----------------------------------------------------------

#[test]
fn the_api_key_reads_from_the_router_plugin_table_beside_the_settings() {
    // Through the config, so this pins WHERE the key is read from and not
    // only how: a table lifted from anywhere else would pass a bare-table
    // assertion just as well.
    let config = crate::config::parse_config(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
    )
    .unwrap();
    let router = enabled_router_table(&config).expect("the enabled table");
    assert_eq!(router_api_key(router), Some("k-123".to_string()));
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

// --- where the stale alert is routed -------------------------------------

#[test]
fn a_usable_stale_alert_channel_is_read_back_as_the_route_verbatim() {
    // The operator names a hermes ROUTE, and the name they typed is what
    // the alert carries: nothing here rewrites, lowercases or defaults it.
    assert_eq!(
        stale_alert_channel(&table("stale_alert_channel = \"priority\"\n")),
        ("priority".to_string(), None)
    );
}

#[test]
fn no_stale_alert_channel_at_all_asks_for_the_default_route_in_silence() {
    // ABSENT IS NOT AN ERROR: the key is optional, and an empty route is
    // how every caller of `hermes_url_for` spells the default route
    // (`/webhooks/pns`). Complaining here would put a config error in
    // front of every operator who never asked to route the alert anywhere.
    assert_eq!(
        stale_alert_channel(&table("type = \"unifi\"\n")),
        (String::new(), None)
    );
}

#[test]
fn a_stale_alert_channel_that_is_not_a_usable_route_complains_and_falls_back() {
    // LOUD-WARD, the same direction `hermes_url_for` falls: a misrouted
    // alert on the default route beats one silently dropped, and the
    // complaint names the CONFIG KEY, because the config is where the
    // operator has to go. The three ways the value can fail are one
    // message, because they are one fix.
    for (setting, quoted) in [
        ("stale_alert_channel = \"\"\n", "\"\""),
        ("stale_alert_channel = \"a/b\"\n", "\"a/b\""),
        ("stale_alert_channel = \"../alert\"\n", "\"../alert\""),
        ("stale_alert_channel = 5\n", "<integer>"),
        ("stale_alert_channel = true\n", "<boolean>"),
    ] {
        let (route, complaint) = stale_alert_channel(&table(setting));
        assert_eq!(route, String::new(), "case: {setting:?}");
        assert_eq!(
            complaint.as_deref(),
            Some(
                format!(
                    "pns: config error (stale_alert_channel = {quoted} in [plugins.router] \
                     is not a usable route name); the stale alert posts to the default route"
                )
                .as_str()
            ),
            "case: {setting:?}"
        );
    }
}
