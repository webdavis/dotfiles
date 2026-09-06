//! The home probe, pinned: reading.

use super::fixtures::*;

// --- parsing the router's listing ---------------------------------------

#[test]
fn every_client_in_the_live_capture_is_read_with_all_three_of_its_fields() {
    let clients = parse_clients(CLIENTS_CAPTURE).expect("the live capture is a listing");
    assert_eq!(clients.len(), 4);
    assert_eq!(
        clients[2],
        Client {
            name: Some("mister".to_string()),
            ipv4: Some("192.168.1.169".to_string()),
            mac: Some("2e:11:ab:6d:b0:4f".to_string()),
        }
    );
    // An UNNAMED client STAYS in the listing now. The router has not
    // identified it, but it still carries the MAC and the address a
    // configured device can match on, and dropping it would answer
    // NotHome for a device sitting right there in the list.
    assert_eq!(
        parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"},{"name":"mister"}]}"#),
        Some(vec![
            Client {
                name: None,
                ipv4: None,
                mac: Some("2E:11:AB:6D:B0:4F".to_string()),
            },
            Client {
                name: Some("mister".to_string()),
                ipv4: None,
                mac: None,
            },
        ])
    );
}

#[test]
fn a_listing_that_is_not_json_is_no_answer_rather_than_an_empty_wifi() {
    // Unparseable and empty must stay distinct: empty means "nobody is
    // on the wifi" and would read as the phone having LEFT.
    assert_eq!(parse_clients("<html>router login</html>"), None);
    assert_eq!(parse_clients(""), None);
}

#[test]
fn a_json_answer_without_the_data_list_is_no_answer() {
    // The auth-failed shape: valid JSON, no clients in it.
    assert_eq!(parse_clients(r#"{"error":"unauthorized"}"#), None);
    assert_eq!(parse_clients(r#"{"data":"not-a-list"}"#), None);
}

#[test]
fn a_parsed_empty_list_is_an_answer_and_not_a_failure() {
    assert_eq!(
        parse_clients(r#"{"offset":0,"limit":200,"count":0,"totalCount":0,"data":[]}"#),
        Some(Vec::new())
    );
}

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

#[test]
fn a_key_that_found_another_client_than_the_verdict_names_says_which_one() {
    // The three keys DISAGREE: the MAC found "mouse" and answers, so the
    // name and the address are pointing at clients this device is not.
    let disagreeing = r#"{"data":[
        {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
        {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"},
        {"name":"dresden","macAddress":"3c:06:30:0f:8a:bf","ipAddress":"192.168.1.169"}]}"#;
    assert_eq!(
        home_reading(parse_clients(disagreeing), &full_identity())
            .keys
            .iter()
            .map(|reading| reading.outcome.clone())
            .collect::<Vec<_>>(),
        vec![
            KeyOutcome::MatchedDevice,
            KeyOutcome::MatchedOtherClient {
                client: "\"mister\"".to_string(),
            },
            KeyOutcome::MatchedOtherClient {
                client: "\"dresden\"".to_string(),
            },
        ]
    );
    // The OTHER client is named by the first field that identifies it,
    // its name, then its MAC, then its address: the router has not
    // identified every client it lists, and "a different client" with no
    // way to tell which one is a diagnostic that cannot be acted on.
    for (listing, client) in [
        (
            r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                {"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.8"}]}"#,
            "\"mouse\"",
        ),
        (
            r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                {"macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.8"}]}"#,
            "\"60:82:46:3c:fb:01\"",
        ),
        (
            r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                {"ipAddress":"192.168.1.8"}]}"#,
            "\"192.168.1.8\"",
        ),
        // PRESENT BUT EMPTY is the field the router has no answer for,
        // the same hole as absent: `matched a different client ""` names
        // nothing an operator can act on, so an empty field falls through
        // to the next one exactly as a missing field does.
        (
            r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                {"name":"","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.8"}]}"#,
            "\"60:82:46:3c:fb:01\"",
        ),
        (
            r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                {"name":"","macAddress":"","ipAddress":"192.168.1.8"}]}"#,
            "\"192.168.1.8\"",
        ),
    ] {
        assert_eq!(
            home_reading(
                parse_clients(listing),
                &identity("device_hostname = \"mister\"\ndevice_ipv4 = \"192.168.1.8\"\n"),
            )
            .keys
            .last()
            .expect("the address is a configured key")
            .outcome,
            KeyOutcome::MatchedOtherClient {
                client: client.to_string(),
            },
            "case: {listing:?}"
        );
    }
}

#[test]
fn a_key_the_winners_own_entry_carries_is_this_device_in_either_listing_order() {
    // ONE physical state, listed two ways. Two clients answer to
    // "mister" and only the second-listed one carries the configured MAC,
    // which is the duplicate-entry case the module docs name. The MAC
    // wins either way and its entry ALSO carries the hostname, so nothing
    // disagrees with anything and the order the router happened to list
    // them in cannot change that.
    let winner_second = r#"{"data":[
        {"name":"mister","ipAddress":"192.168.1.7","macAddress":"60:82:46:3c:fb:01"},
        {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"}]}"#;
    let winner_first = r#"{"data":[
        {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
        {"name":"mister","ipAddress":"192.168.1.7","macAddress":"60:82:46:3c:fb:01"}]}"#;
    for listing in [winner_second, winner_first] {
        let reading = home_reading(
            parse_clients(listing),
            &identity(
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mister\"\n",
            ),
        );
        assert_eq!(
            reading
                .keys
                .iter()
                .map(|reading| reading.outcome.clone())
                .collect::<Vec<_>>(),
            vec![KeyOutcome::MatchedDevice, KeyOutcome::MatchedDevice],
            "case: {listing:?}"
        );
        assert_eq!(
            stale_identifiers(&reading),
            None,
            "a key the winner's own entry carries is not stale: {listing:?}"
        );
    }
}

#[test]
fn not_home_says_every_key_matched_no_client_and_unknown_says_nothing_at_all() {
    // The NotHome line names no identifier, so the evidence is the only
    // place an operator can see WHICH keys were looked for and that every
    // one of them came up empty.
    let reading = home_reading(
        parse_clients(
            r#"{"totalCount":1,"data":[{"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.248"}]}"#,
        ),
        &full_identity(),
    );
    assert_eq!(reading.presence, HomePresence::NotHome);
    assert_eq!(
        reading
            .keys
            .iter()
            .map(|reading| (reading.key, reading.value.as_str(), reading.outcome.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                DeviceKey::Mac,
                "2e:11:ab:6d:b0:4f",
                KeyOutcome::MatchedNothing
            ),
            (DeviceKey::Hostname, "mister", KeyOutcome::MatchedNothing),
            (DeviceKey::Ipv4, "192.168.1.169", KeyOutcome::MatchedNothing),
        ]
    );
    // NOTHING WAS SEARCHED for an Unknown, so it carries no readings:
    // "matched no client" is a claim about a listing, and no listing
    // arrived.
    let unknown = home_reading(parse_clients("<html>router login</html>"), &full_identity());
    assert_eq!(unknown.presence, HomePresence::Unknown);
    assert!(unknown.keys.is_empty(), "got: {:?}", unknown.keys);
}

#[test]
fn a_complete_listing_no_key_matched_is_not_home_and_anything_less_is_unknown() {
    let device = full_identity();
    // NotHome needs a COMPLETE listing that none of the three keys found:
    // the wifi answered, and the device was not on it.
    assert_eq!(
        home_reading(
            parse_clients(
                r#"{"totalCount":1,"data":[{"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.248"}]}"#
            ),
            &device
        ).presence,
        HomePresence::NotHome
    );
    assert_eq!(
        home_reading(parse_clients(r#"{"totalCount":0,"data":[]}"#), &device).presence,
        HomePresence::NotHome
    );
    // Unreachable, unparseable and INCOMPLETE all stay Unknown. The
    // consumers act on transitions, so inventing a departure out of a
    // page the device could be beyond would fire a false one.
    for no_answer in [
        "<html>router login</html>",
        r#"{"error":"unauthorized"}"#,
        r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"mouse"}]}"#,
    ] {
        assert_eq!(
            home_reading(parse_clients(no_answer), &device).presence,
            HomePresence::Unknown,
            "case: {no_answer:?}"
        );
    }
}

// --- the seam, end to end ------------------------------------------------

struct FakeRouter(Option<&'static str>);
impl Router for FakeRouter {
    fn clients_json(&self) -> Option<String> {
        self.0.map(str::to_string)
    }
}

#[test]
fn one_reading_runs_fetch_parse_judge_in_order() {
    let device = identity("device_hostname = \"mister\"\n");
    assert_eq!(
        read_home(&FakeRouter(Some(CLIENTS_CAPTURE)), &device).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }
    );
    assert_eq!(
        read_home(&FakeRouter(None), &device).presence,
        HomePresence::Unknown
    );
}
