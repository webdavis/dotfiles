use super::*;

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
