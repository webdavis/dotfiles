use super::*;
use crate::home_report::verdict_line;

/// The rows a reading earns, flattened to `(mark, sentence)` pairs.
///
/// THE MARK IS ASSERTED, not a glyph: these are the report's SHAPE, and what
/// each mark looks like on a terminal belongs to the renderer's own tests.
fn marked(reading: &HomeReading, news: Option<&Staleness>) -> Vec<(Mark, String)> {
    rows(reading, news)
        .into_iter()
        .map(|item| match item {
            pns_domain::doctor::Item::Row { mark, text } => (mark, text),
            pns_domain::doctor::Item::Section { title, .. } => {
                panic!("the home rows carry no section: {title}")
            }
        })
        .collect()
}

/// Just the sentences, in order.
fn said(reading: &HomeReading, news: Option<&Staleness>) -> Vec<String> {
    marked(reading, news)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
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
fn each_presence_verdict_reports_its_own_sentence() {
    // The Home sentence NAMES the identifier that answered and the value it
    // answered with, which is the only observable difference precedence
    // makes: the operator can see WHICH key spoke without a per-key
    // breakdown that would expose the probe's internals.
    //
    // PINNED ON THE PRODUCER, not on the styled report, so the words stay
    // covered whatever the layout around them becomes.
    assert_eq!(
        verdict_line(&HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: "2e:11:ab:6d:b0:4f".to_string(),
        }),
        "on the home network, matched by device_mac \"2e:11:ab:6d:b0:4f\""
    );
    assert_eq!(
        verdict_line(&HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }),
        "on the home network, matched by device_hostname \"mister\""
    );
    // THE VALUE IS ESCAPED, exactly as `spell` escapes a config value next
    // door: a client name carrying a quote or an ESC byte reaches stdout as
    // its escape, never as the byte. The two lines above are the proof this
    // costs nothing to read: debug-quoting a plain string is the same
    // quoted form it always had.
    assert_eq!(
        verdict_line(&HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mist\"er\u{1b}[2J".to_string(),
        }),
        "on the home network, matched by device_hostname \"mist\\\"er\\u{1b}[2J\""
    );
    assert_eq!(
        verdict_line(&HomePresence::NotHome),
        "NOT on the home network: no configured identifier matched a client"
    );
    assert_eq!(
        verdict_line(&HomePresence::Unknown),
        concat!(
            "unknown: the router returned no readable client list, so nothing was established; ",
            "check router_url and api_key in [plugins.router] ",
            "(a rejected key reads the same here as an unreachable router)"
        )
    );
}

#[test]
fn a_verdict_carries_a_mark_that_says_which_of_the_three_it_is() {
    // UNKNOWN IS A WARNING, NOT A VERDICT. The router did not answer, so
    // nothing was established either way, and a reader who skims the marks
    // must not read it as "not home". A warning also withholds the report's
    // all-clear, which is what makes an unread router visible at all.
    //
    // AND NOTHING IS `Bad`: this reading cannot move the doctor's exit code,
    // because an unread router breaks no notification path.
    for (presence, mark) in [
        (
            HomePresence::Home {
                matched_by: DeviceKey::Mac,
                value: "2e:11:ab:6d:b0:4f".to_string(),
            },
            Mark::Good,
        ),
        (HomePresence::NotHome, Mark::Note),
        (HomePresence::Unknown, Mark::Warn),
    ] {
        let reading = verdict_only(presence.clone());
        assert_eq!(
            marked(&reading, None).first().expect("a verdict row").0,
            mark,
            "{presence:?}"
        );
    }
}

#[test]
fn the_verdict_row_names_the_probe_and_a_reading_with_no_keys_adds_no_rows() {
    // ONE ROW AND NO MORE. The verdict stands alone under a shared section
    // heading, so it names the probe itself; an evidence row over nothing
    // would read as a key that failed to load.
    assert_eq!(
        said(&verdict_only(HomePresence::NotHome), None),
        ["home: NOT on the home network: no configured identifier matched a client"]
    );
}

#[test]
fn the_evidence_under_the_verdict_says_what_each_key_found_escaping_the_label() {
    // Every CONFIGURED key gets a row, whatever it found, because the
    // diagnostic's job is to show the disagreement rather than the
    // winner. The ROUTER is not the operator: the client label is the one
    // string on these rows nobody here typed, so it reaches a terminal
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
    let rows = marked(&reading, stale_identifiers(&reading).as_ref());
    // THE EVIDENCE IS `Detail`, which is what makes it read as the verdict's
    // explanation rather than as three more findings.
    let expected = [
        (
            Mark::Good,
            "home: on the home network, matched by device_hostname \"mister\"".to_string(),
        ),
        (
            Mark::Detail,
            "device_mac        \"2e:11:ab:6d:b0:4f\"   matched no client".to_string(),
        ),
        (
            Mark::Detail,
            "device_hostname   \"mister\"   matched the client the verdict names".to_string(),
        ),
        (
            Mark::Detail,
            "device_ipv4       \"192.168.1.8\"   matched a different client \
             \"mo\\\"use\\u{1b}[2J\""
                .to_string(),
        ),
        (
            Mark::Warn,
            "an identifier looks stale: device_mac, device_ipv4 disagree with device_hostname"
                .to_string(),
        ),
    ];
    assert_eq!(rows, expected);
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
    let warned = said(&reading, stale_identifiers(&reading).as_ref()).join("\n");
    assert!(warned.contains(
        "an identifier looks stale: device_hostname, device_ipv4 disagree with device_mac"
    ));
    // A REPEAT keeps every evidence row and drops the alert-shaped one:
    // a hand-run report always tells the whole truth, and only the warning
    // is said once.
    let quiet = said(&reading, None).join("\n");
    assert!(!quiet.contains("looks stale"), "{quiet}");
    assert!(quiet.contains("device_hostname"), "{quiet}");
    assert!(quiet.contains("device_ipv4"), "{quiet}");
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
        said(&one_key, stale_identifiers(&one_key).as_ref())
            .last()
            .expect("a staleness row"),
        "an identifier looks stale: device_ipv4 disagrees with device_mac"
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

#[test]
fn a_probe_nobody_set_up_is_a_note_and_one_set_up_wrong_is_a_warning() {
    // GRADING A CHOICE AS A FAULT is how a reader learns to skim the marks: a
    // machine with no [plugins.router] table never asked for a home reading,
    // and a warning there would withhold the report's all-clear on every run
    // forever. A table that WAS written and does not work is an edit waiting
    // to be made, so it is a warning.
    //
    // AND NOTHING IS `Bad`: none of these breaks a notification path.
    let mark_of = |failure: &SetupFailure| match setup_row(failure) {
        pns_domain::doctor::Item::Row { mark, .. } => mark,
        other => panic!("a setup failure is a row: {other:?}"),
    };
    for failure in [
        SetupFailure::NoConfigFile,
        SetupFailure::NoRouterPlugin,
        SetupFailure::RouterDisabled,
    ] {
        assert_eq!(mark_of(&failure), Mark::Note, "{failure:?}");
    }
    for failure in [
        SetupFailure::ConfigError("refused".to_string()),
        SetupFailure::NoType,
        SetupFailure::UnknownType("asus".to_string()),
        SetupFailure::InvalidRouterTable,
        SetupFailure::NoDeviceIdentifier,
        SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Ipv4,
            found: "\"1.2.3\"".to_string(),
        },
        SetupFailure::NoApiKey,
    ] {
        assert_eq!(mark_of(&failure), Mark::Warn, "{failure:?}");
    }
}
