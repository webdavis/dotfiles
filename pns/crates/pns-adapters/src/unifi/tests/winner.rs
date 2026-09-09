use super::*;

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
