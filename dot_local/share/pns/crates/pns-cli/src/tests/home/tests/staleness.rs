//! The home probe, pinned: staleness.

use super::fixtures::*;

// --- the staleness ------------------------------------------------------

#[test]
fn a_staleness_is_a_home_verdict_with_a_key_pointing_somewhere_else() {
    // The MAC found "mouse" and answers; the name found the OTHER client
    // and the address found nobody. Both of those disagree with the
    // winner, which is the whole state this detects.
    let disagreeing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#;
    let staleness = stale_identifiers(&home_reading(parse_clients(disagreeing), &full_identity()))
        .expect("two keys point away from the client the MAC named");
    assert_eq!(staleness.winner, DeviceKey::Mac);
    assert_eq!(
        staleness
            .disagreeing
            .iter()
            .map(|reading| (reading.key, reading.outcome.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                DeviceKey::Hostname,
                KeyOutcome::MatchedOtherClient {
                    client: "\"mister\"".to_string(),
                }
            ),
            (DeviceKey::Ipv4, KeyOutcome::MatchedNothing),
        ]
    );
    // KEYS THAT AGREE are not a staleness, ONE key has nothing to
    // disagree with, and away is not stale: every key matching nothing is
    // what NotHome IS, and an Unknown searched nothing at all.
    for (listing, device, case) in [
        (
            CLIENTS_CAPTURE,
            full_identity(),
            "every key found the phone",
        ),
        (
            r#"{"data":[{"name":"mister"}]}"#,
            identity("device_hostname = \"mister\"\n"),
            "one configured key",
        ),
        (
            r#"{"totalCount":1,"data":[{"name":"mouse"}]}"#,
            full_identity(),
            "not home",
        ),
        ("<html>router login</html>", full_identity(), "unknown"),
    ] {
        assert_eq!(
            stale_identifiers(&home_reading(parse_clients(listing), &device)),
            None,
            "case: {case}"
        );
    }
}

/// The staleness in one listing judged against one table, which is the
/// only way an episode identity is ever spelled.
fn episode(listing: &str, device: &str) -> String {
    episode_id(
        &stale_identifiers(&home_reading(parse_clients(listing), &identity(device)))
            .expect("a staleness"),
    )
}

#[test]
fn an_episode_identity_spells_the_state_and_never_the_values_that_moved() {
    // The MAC answers, the name is pointing at another client and the
    // address is pointing at nobody: THAT is the state, and its spelling
    // is the winner plus each disagreeing key and what it found.
    assert_eq!(
        episode(
            r#"{"data":[
                    {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
                    {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#,
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister\"\n\
             device_ipv4 = \"192.168.1.169\"\n",
        ),
        "device_mac device_hostname=other device_ipv4=none"
    );
    // DHCP CHURN IS NOT NEWS. Every value here moved (a different stale
    // address, a different client under the name, a different label on
    // it) while the state did not, so the operator is not told the same
    // thing twice.
    assert_eq!(
        episode(
            r#"{"data":[
                    {"name":"kite","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.9.9"},
                    {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#,
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"kite\"\n\
             device_ipv4 = \"10.0.0.5\"\n",
        ),
        "device_mac device_hostname=other device_ipv4=none"
    );
}

#[test]
fn a_changed_stale_set_outcome_or_winner_each_spell_a_different_identity() {
    // One listing, four ways the STATE can differ under it. The MAC and
    // the name both point at "mouse" in the first case, so only the
    // address disagrees.
    let listing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#;
    let mut identities = vec![
        episode(
            listing,
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mouse\"\n\
             device_ipv4 = \"192.168.1.169\"\n",
        ),
        // A DIFFERENT KEY ANSWERS, over the same one stale address: the
        // operator is now home on the strength of a label rather than the
        // hardware, which is a weaker footing and its own news.
        episode(
            listing,
            "device_hostname = \"mouse\"\n\
             device_ipv4 = \"192.168.1.169\"\n",
        ),
        // The address STOPPED matching nothing and started matching
        // somebody else.
        episode(
            listing,
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mouse\"\n\
             device_ipv4 = \"192.168.1.7\"\n",
        ),
        // The name JOINED the stale set.
        episode(
            listing,
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister\"\n\
             device_ipv4 = \"192.168.1.169\"\n",
        ),
    ];
    assert_eq!(
        identities,
        vec![
            "device_mac device_ipv4=none",
            "device_hostname device_ipv4=none",
            "device_mac device_ipv4=other",
            "device_mac device_hostname=other device_ipv4=none",
        ]
    );
    identities.sort();
    identities.dedup();
    assert_eq!(identities.len(), 4, "each state is its own news");
}

#[test]
fn a_staleness_is_news_only_when_its_identity_differs_from_the_remembered_one() {
    // The dedupe is over VALUES, not over readings: the same state read
    // fifty times is one piece of news, and a state that RESOLVED is news
    // to nobody, because the operator was told about a disagreement that
    // is no longer there.
    for (remembered, current, news, case) in [
        (
            None,
            Some("device_mac device_ipv4=none"),
            true,
            "first sighting",
        ),
        (
            Some("device_mac device_ipv4=none"),
            Some("device_mac device_ipv4=none"),
            false,
            "the same state again",
        ),
        (
            Some("device_mac device_ipv4=none"),
            Some("device_mac device_ipv4=other"),
            true,
            "the state moved",
        ),
        (
            Some("device_mac device_ipv4=none"),
            None,
            false,
            "the disagreement resolved",
        ),
        (None, None, false, "nothing to say"),
    ] {
        assert_eq!(is_new_staleness(remembered, current), news, "case: {case}");
    }
}

#[test]
fn the_stale_warning_is_one_sentence_that_agrees_with_the_keys_it_names() {
    // THE SENTENCE A CONSUMER ACTS ON, which is why it is a function of
    // its own: the diagnostic prints it and the delivered alert carries
    // it, and one spelling is what keeps the terminal line and the
    // notification from drifting apart.
    let two_disagree = home_reading(
        parse_clients(CLIENTS_CAPTURE),
        &identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister-2\"\n\
             device_ipv4 = \"192.168.1.248\"\n",
        ),
    );
    assert_eq!(
        stale_warning(&stale_identifiers(&two_disagree).expect("two keys point away")),
        "home: an identifier looks stale: device_hostname, device_ipv4 \
         disagree with device_mac"
    );
    // ONE disagreeing key is one key: the verb agrees with what it names.
    let one_disagrees = home_reading(
        parse_clients(CLIENTS_CAPTURE),
        &identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_ipv4 = \"192.168.1.248\"\n",
        ),
    );
    assert_eq!(
        stale_warning(&stale_identifiers(&one_disagrees).expect("one key points away")),
        "home: an identifier looks stale: device_ipv4 disagrees with device_mac"
    );
}
