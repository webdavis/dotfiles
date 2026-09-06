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
