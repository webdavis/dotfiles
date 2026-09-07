use super::*;

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
