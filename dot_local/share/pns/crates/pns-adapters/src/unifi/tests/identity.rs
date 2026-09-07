use super::*;

// --- the verdict ---------------------------------------------------------

#[test]
fn a_hostname_match_is_exact_so_a_sibling_device_cannot_answer_for_the_phone() {
    let device = identity("device_hostname = \"mister\"\n");
    assert_eq!(
        home_reading(only_names(&["dresden", "mister"]), &device).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }
    );
    // A substring match would let "mister-2" answer, and a case-blind
    // one would let "MISTER": both are other devices on this wifi.
    assert_eq!(
        home_reading(only_names(&["mister-2", "MISTER"]), &device).presence,
        HomePresence::NotHome
    );
}

#[test]
fn a_mac_only_identity_reads_home_through_the_one_normalized_spelling() {
    // The operator copied it off a sticker (uppercase, dashes) and the
    // UDR answers lowercase with colons. BOTH SIDES go through the same
    // normalizer, so those are one value and not two.
    let device = identity("device_mac = \"2E-11-AB-6D-B0-4F\"\n");
    let matched = HomePresence::Home {
        matched_by: DeviceKey::Mac,
        value: "2e:11:ab:6d:b0:4f".to_string(),
    };
    assert_eq!(
        home_reading(parse_clients(CLIENTS_CAPTURE), &device).presence,
        matched
    );
    // And an UNNAMED client answers on its MAC, which is what the old
    // name filter would have thrown away.
    assert_eq!(
        home_reading(
            parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"}]}"#),
            &device
        )
        .presence,
        matched
    );
    // A MAC the probe cannot read is a client that matches nothing.
    assert_eq!(
        home_reading(
            parse_clients(r#"{"data":[{"macAddress":"nonsense"},{"name":"dresden"}]}"#),
            &device
        )
        .presence,
        HomePresence::NotHome
    );
}

#[test]
fn an_ipv4_only_identity_reads_home_against_the_client_carrying_that_address() {
    // ADDRESSES are compared, never the texts they were written as: the
    // client's `ipAddress` is parsed the same way the config's value was.
    let device = identity("device_ipv4 = \"192.168.1.169\"\n");
    assert_eq!(
        home_reading(parse_clients(CLIENTS_CAPTURE), &device).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Ipv4,
            value: "192.168.1.169".to_string(),
        }
    );
    // A client whose address is missing or is not an IPv4 is a client
    // that matches nothing. The ROUTER is not the operator: its entries
    // are read for what they hold, never refused for what they lack.
    assert_eq!(
        home_reading(
            parse_clients(r#"{"data":[{"ipAddress":"not-an-address"},{"name":"dresden"}]}"#),
            &device
        )
        .presence,
        HomePresence::NotHome
    );
}

#[test]
fn any_one_configured_key_matching_reads_home_while_the_others_match_nothing() {
    // A key that matches NOTHING is skipped, never a failure: the device
    // is home on the strength of the one key that answered, whichever it
    // is. Each listing below carries exactly one of the three.
    for (listing, matched_by, value) in [
        (
            r#"{"data":[{"name":"dresden","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.7"}]}"#,
            DeviceKey::Mac,
            "2e:11:ab:6d:b0:4f",
        ),
        (
            r#"{"data":[{"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"}]}"#,
            DeviceKey::Hostname,
            "mister",
        ),
        (
            r#"{"data":[{"name":"dresden","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.169"}]}"#,
            DeviceKey::Ipv4,
            "192.168.1.169",
        ),
    ] {
        assert_eq!(
            home_reading(parse_clients(listing), &full_identity()).presence,
            HomePresence::Home {
                matched_by,
                value: value.to_string(),
            },
            "case: {listing:?}"
        );
    }
}
