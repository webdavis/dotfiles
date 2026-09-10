use super::*;

#[test]
fn the_same_stale_state_alerts_once_and_a_returning_one_alerts_again() {
    // ONE MEMORY, ONE DECISION: the file that decides whether the sentence is
    // printed is the file that decides whether it is delivered, so the alert
    // cannot fire on a state the operator was already told about, and cannot
    // stay silent about one they were not.
    let sandbox = Sandbox::new("home-stale-alert-once");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let home = || {
        let mut probe = home_probe(&sandbox);
        run(&mut probe);
    };

    home();
    assert_eq!(alerts(&sandbox).len(), 1, "the first sighting is news");
    home();
    assert_eq!(
        alerts(&sandbox).len(),
        1,
        "the same state again is news to nobody"
    );
    // RESOLVED is not an alert either: an all-clear for a warning the
    // operator may never have read is one more thing to read.
    router.set_listing(KEYS_AGREE);
    home();
    assert_eq!(alerts(&sandbox).len(), 1, "a resolved state says nothing");
    // And the episode coming BACK is news again, which is the half a memory
    // that was never cleared would lose.
    router.set_listing(KEYS_DISAGREE);
    home();
    assert_eq!(
        alerts(&sandbox).len(),
        2,
        "the state returned, so it is news again"
    );
}

#[test]
fn only_a_home_reading_alerts_and_the_sensor_is_never_a_destination() {
    // AWAY AND UNREADABLE HAVE NO OPINION about the identifiers, one layer up
    // from the memory rule they already obey: every key matching nothing is
    // what being out of the house IS, and a router timeout searched nothing at
    // all. Delivering on either would page the operator for leaving the house.
    let sandbox = Sandbox::new("home-stale-alert-not-home");
    count_alerts(&sandbox);
    // A RECORDING stub under the sensor's own name, so a router that had
    // somehow become a leg leaves a trace instead of exec'ing nothing.
    sandbox.stub_channel(
        "router",
        &format!("cat >\"{}/router.event\"", sandbox.display()),
    );
    let router = RouterStub::start(KEYS_AWAY);
    sandbox.write_config(&stale_config(&router.url()));
    let home = || {
        let mut probe = home_probe(&sandbox);
        run(&mut probe);
    };

    home();
    assert!(alerts(&sandbox).is_empty(), "away is not a staleness");
    router.set_listing(NO_LISTING);
    home();
    assert!(
        alerts(&sandbox).is_empty(),
        "an unreadable answer is not a staleness"
    );
    // The same probe on a Home reading DOES alert, which is what makes the
    // two silences above assertions rather than a test that never armed.
    router.set_listing(KEYS_DISAGREE);
    home();
    assert_eq!(alerts(&sandbox).len(), 1, "a Home reading alerts");
    assert!(
        !sandbox.fired("router"),
        "the roster registers router as a SENSOR: an input carries no routing, \
         so the alert ABOUT its reading can never be delivered back to it"
    );
}

#[test]
fn the_alert_carries_no_secret_and_no_raw_router_text() {
    // TWO SECRETS ARE IN REACH on this path now: the router's own api_key,
    // which `home_mode` reads, and the hermes signing key, which the dispatch
    // reads. Neither may ride an event to a channel or a line to a terminal.
    let sandbox = Sandbox::new("home-stale-alert-secrets");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE_HOSTILE_LABEL);
    sandbox.write_config(&format!(
        "[plugins.hermes]\nenabled = true\nkey = \"hermes-signing-secret\"\n{}",
        router_table(&router.url())
    ));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    let delivered = std::fs::read_to_string(sandbox.path("hermes.events")).expect("an alert");
    for secret in ["k-123", "hermes-signing-secret"] {
        assert!(
            !delivered.contains(secret),
            "the delivered event carries {secret:?}: {delivered}"
        );
        assert!(
            !stderr(&output).contains(secret),
            "stderr carries {secret:?}: {}",
            stderr(&output)
        );
        assert!(
            !stdout(&output).contains(secret),
            "the diagnostic carries {secret:?}: {}",
            stdout(&output)
        );
    }
    // THE ROUTER'S OWN STRINGS keep slice 4's escape in the terminal, and
    // reach the alert body not at all: the sentence is built from config KEY
    // NAMES, so a client label cannot ride it out to a channel.
    assert!(
        stdout(&output).contains(concat!(
            "device_ipv4       \"192.168.1.248\"   matched a different client ",
            "\"mo\\\"use\\u{1b}[2J\"",
        )),
        "the evidence escapes the label: {}",
        stdout(&output)
    );
    assert_eq!(alerts(&sandbox)[0]["detail"], STALE_WARNING);
}

#[test]
fn an_unusable_stale_alert_route_complains_and_still_delivers_the_alert() {
    // LOUD-WARD: a config typo in the ROUTE must not be what silences the
    // warning that route was configured for. The complaint names the config
    // key, because that is the file the operator has to open, and the alert
    // still goes out on the route they would have had by writing nothing.
    let sandbox = Sandbox::new("home-stale-alert-bad-route");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&format!(
        "{}stale_alert_channel = \"../alert\"\n",
        stale_config(&router.url())
    ));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    assert_eq!(
        stderr(&output).trim_end(),
        "pns: config error (stale_alert_channel = \"../alert\" in [plugins.router] is not a \
         usable route name); the stale alert posts to the default route"
    );
    assert_eq!(
        alerts(&sandbox).len(),
        1,
        "the alert is still delivered, on the default route"
    );
    assert_eq!(
        stdout(&output),
        format!("{STALE_EVIDENCE}\n{STALE_WARNING_ROW}\n"),
        "and the diagnostic itself is untouched"
    );
}
