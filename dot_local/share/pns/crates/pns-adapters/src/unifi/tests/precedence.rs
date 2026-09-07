use super::*;

#[test]
fn on_keys_matching_different_clients_the_verdict_names_the_strongest() {
    // The three keys DISAGREE here: each points at a different client.
    // The strongest one that matched anything is the one that names the
    // device, because a MAC is the device itself, a name is a label the
    // operator can reuse, and an address is only today's lease.
    let disagreeing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"},
            {"name":"dresden","macAddress":"3c:06:30:0f:8a:bf","ipAddress":"192.168.1.169"}]}"#;
    assert_eq!(
        home_reading(parse_clients(disagreeing), &full_identity()).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: "2e:11:ab:6d:b0:4f".to_string(),
        }
    );
    // Drop the MAC and the name is next in line; drop that too and the
    // address answers on its own.
    assert_eq!(
        home_reading(
            parse_clients(disagreeing),
            &identity("device_hostname = \"mister\"\ndevice_ipv4 = \"192.168.1.169\"\n")
        )
        .presence,
        HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }
    );
    assert_eq!(
        home_reading(
            parse_clients(disagreeing),
            &identity("device_ipv4 = \"192.168.1.169\"\n")
        )
        .presence,
        HomePresence::Home {
            matched_by: DeviceKey::Ipv4,
            value: "192.168.1.169".to_string(),
        }
    );
}

// --- the per-key reading -------------------------------------------------

#[test]
fn a_reading_carries_one_entry_per_configured_key_in_precedence_order() {
    let reading = home_reading(parse_clients(CLIENTS_CAPTURE), &full_identity());
    assert_eq!(
        reading
            .keys
            .iter()
            .map(|reading| (reading.key, reading.value.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (DeviceKey::Mac, "2e:11:ab:6d:b0:4f"),
            (DeviceKey::Hostname, "mister"),
            (DeviceKey::Ipv4, "192.168.1.169"),
        ]
    );
    // An UNSET key is skipped rather than reported as absent, and the
    // ORDER is precedence's, not the order the table happened to list
    // them in: this table names the address first.
    let two_keys = home_reading(
        parse_clients(CLIENTS_CAPTURE),
        &identity("device_ipv4 = \"192.168.1.169\"\ndevice_hostname = \"mister\"\n"),
    );
    assert_eq!(
        two_keys
            .keys
            .iter()
            .map(|reading| reading.key)
            .collect::<Vec<_>>(),
        vec![DeviceKey::Hostname, DeviceKey::Ipv4]
    );
}

#[test]
fn every_key_that_found_the_client_the_verdict_names_is_marked_as_this_device() {
    // All three keys point at the phone of the live capture, so all three
    // found the SAME entry: the verdict names the strongest, and no key
    // disagrees with it.
    let reading = home_reading(parse_clients(CLIENTS_CAPTURE), &full_identity());
    assert_eq!(
        reading
            .keys
            .iter()
            .map(|reading| reading.outcome.clone())
            .collect::<Vec<_>>(),
        vec![
            KeyOutcome::MatchedDevice,
            KeyOutcome::MatchedDevice,
            KeyOutcome::MatchedDevice,
        ]
    );
    assert_eq!(
        reading.presence,
        HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: "2e:11:ab:6d:b0:4f".to_string(),
        }
    );
}
