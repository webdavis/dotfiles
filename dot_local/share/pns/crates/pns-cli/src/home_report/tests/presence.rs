use super::*;

// --- the reported lines, pinned so the words match the verdict -----------

/// A verdict with no evidence under it, which is what an Unknown really
/// is: the verdict LINE is what these cases pin, one sentence at a time.
fn verdict_only(presence: HomePresence) -> HomeReading {
    HomeReading {
        presence,
        keys: Vec::new(),
    }
}

#[test]
fn each_presence_verdict_reports_its_own_line() {
    // The Home line NAMES the identifier that answered and the value it
    // answered with, which is the only observable difference precedence
    // makes: the operator can see WHICH key spoke without a per-key
    // breakdown that would expose the probe's internals.
    assert_eq!(
        report(
            &verdict_only(HomePresence::Home {
                matched_by: DeviceKey::Mac,
                value: "2e:11:ab:6d:b0:4f".to_string(),
            }),
            None
        ),
        "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")"
    );
    assert_eq!(
        report(
            &verdict_only(HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }),
            None
        ),
        "home: on the home network (matched by device_hostname \"mister\")"
    );
    // THE VALUE IS ESCAPED, exactly as `spell` escapes a config value next
    // door: a client name carrying a quote or an ESC byte reaches stdout as
    // its escape, never as the byte. The two lines above are the proof this
    // costs nothing to read: debug-quoting a plain string is the same
    // quoted form it always had.
    assert_eq!(
        report(
            &verdict_only(HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mist\"er\u{1b}[2J".to_string(),
            }),
            None
        ),
        "home: on the home network (matched by device_hostname \"mist\\\"er\\u{1b}[2J\")"
    );
    assert_eq!(
        report(&verdict_only(HomePresence::NotHome), None),
        "home: NOT on the home network (no configured identifier matched a client)"
    );
    assert_eq!(
        report(&verdict_only(HomePresence::Unknown), None),
        "home: unknown (router unreachable or its answer unreadable)"
    );
}

#[test]
fn the_evidence_under_the_verdict_says_what_each_key_found_escaping_the_label() {
    // Every CONFIGURED key gets a line, whatever it found, because the
    // diagnostic's job is to show the disagreement rather than the
    // winner. The ROUTER is not the operator: the client label is the one
    // string on these lines nobody here typed, so it reaches a terminal
    // as its escape exactly as the matched value does.
    let listing = r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
            {"name":"mo\"use\u001b[2J","ipAddress":"192.168.1.8"}]}"#;
    let reading = home_reading(
        parse_clients(listing),
        &identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister\"\n\
             device_ipv4 = \"192.168.1.8\"\n",
        ),
    );
    assert_eq!(
        report(&reading, stale_identifiers(&reading).as_ref()),
        "home: on the home network (matched by device_hostname \"mister\")\n\
         home:   device_mac \"2e:11:ab:6d:b0:4f\" matched no client\n\
         home:   device_hostname \"mister\" matched the client the verdict names\n\
         home:   device_ipv4 \"192.168.1.8\" matched a different client \
         \"mo\\\"use\\u{1b}[2J\"\n\
         home: an identifier looks stale: device_mac, device_ipv4 disagree with \
         device_hostname"
    );
}

#[test]
fn the_staleness_line_names_the_disagreeing_keys_and_prints_only_when_it_is_news() {
    // The operator's own case: the MAC still names the phone, the name
    // key has gone stale against a client that left, and the address is
    // now somebody else's lease.
    let reading = home_reading(
        parse_clients(CLIENTS_CAPTURE),
        &identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister-2\"\n\
             device_ipv4 = \"192.168.1.248\"\n",
        ),
    );
    let evidence = "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")\n\
         home:   device_mac \"2e:11:ab:6d:b0:4f\" matched the client the verdict names\n\
         home:   device_hostname \"mister-2\" matched no client\n\
         home:   device_ipv4 \"192.168.1.248\" matched a different client \"mouse\"";
    assert_eq!(
        report(&reading, stale_identifiers(&reading).as_ref()),
        format!(
            "{evidence}\nhome: an identifier looks stale: device_hostname, device_ipv4 \
             disagree with device_mac"
        )
    );
    // A REPEAT keeps every evidence line and drops the alert-shaped one:
    // a hand-run diagnostic always tells the whole truth, and only the
    // warning is said once.
    assert_eq!(report(&reading, None), evidence);
    // ONE disagreeing key is one key: the sentence agrees with what it
    // is naming.
    let one_key = home_reading(
        parse_clients(CLIENTS_CAPTURE),
        &identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_ipv4 = \"192.168.1.248\"\n",
        ),
    );
    assert_eq!(
        report(&one_key, stale_identifiers(&one_key).as_ref())
            .lines()
            .last()
            .expect("a staleness line"),
        "home: an identifier looks stale: device_ipv4 disagrees with device_mac"
    );
}

#[test]
fn every_setup_failure_line_names_what_to_look_at() {
    for (failure, needle) in [
        (SetupFailure::NoConfigFile, "no config file"),
        (
            SetupFailure::ConfigError("bad at line 3".to_string()),
            "bad at line 3",
        ),
        (SetupFailure::NoRouterPlugin, "[plugins.router]"),
        (SetupFailure::RouterDisabled, "[plugins.router]"),
        (SetupFailure::NoType, "type"),
        (SetupFailure::UnknownType("asus".to_string()), "asus"),
        (SetupFailure::InvalidRouterTable, "router_url"),
        (SetupFailure::NoDeviceIdentifier, "device_hostname"),
        (
            SetupFailure::InvalidDeviceKey {
                key: DeviceKey::Ipv4,
                found: "\"1.2.3\"".to_string(),
            },
            "device_ipv4",
        ),
        (SetupFailure::NoApiKey, "api_key"),
    ] {
        let line = setup_report(&failure);
        assert!(line.starts_with("home: "), "case {failure:?}: {line}");
        assert!(line.contains(needle), "case {failure:?}: {line}");
    }
}
