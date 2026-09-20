use super::*;

#[test]
fn the_home_rows_always_show_the_evidence_and_warn_once_per_stale_state() {
    // THE MEMORY IS DURABLE, which is the one edge worth an end-to-end case:
    // the dedupe is only worth anything if a LATER run of the binary reads
    // what this one wrote.
    let sandbox = Sandbox::new("home-staleness");
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let home = || {
        let mut probe = home_probe(&sandbox);
        home_rows(&stdout(&run(&mut probe)))
    };
    let memory = || {
        use rusqlite::OptionalExtension;
        stored_records::at(&sandbox.path(".local/state/pns/pns.db"))
            .query_row("SELECT body FROM staleness WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .expect("the stored episode")
    };

    assert_eq!(home(), stale_evidence_warned());
    assert_eq!(
        memory().expect("the episode is remembered").trim(),
        "device_mac device_hostname=none device_ipv4=other"
    );
    // A REPEAT still tells the whole truth and says the warning no more.
    assert_eq!(home(), stale_evidence());
    // RESOLVED: the memory is forgotten rather than left to suppress the next
    // episode, and the state is news again when it comes back.
    router.set_listing(KEYS_AGREE);
    assert_eq!(
        home(),
        rewritten(
            &stale_evidence(),
            &[
                ("matched no client", "matched the client the verdict names"),
                (
                    "matched a different client \"mouse\"",
                    "matched the client the verdict names"
                ),
            ]
        )
    );
    assert!(memory().is_none(), "a resolved episode is forgotten");
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), stale_evidence_warned());

    // AWAY IS NOT RESOLVED. Leaving the house says nothing about the
    // identifiers: every key matches nothing because the device is not on the
    // wifi, so the live episode survives the trip and the homecoming is quiet.
    // Without this the warning is once per HOMECOMING, which for a phone is
    // once a day.
    router.set_listing(KEYS_AWAY);
    assert_eq!(
        home(),
        rewritten(
            &stale_evidence(),
            &[
                (
                    "on the home network, matched by device_mac \"2e:11:ab:6d:b0:4f\"",
                    "NOT on the home network: no configured identifier matched a client"
                ),
                ("matched the client the verdict names", "matched no client"),
                ("matched a different client \"mouse\"", "matched no client"),
            ]
        )
    );
    assert!(
        memory().is_some(),
        "leaving the house does not resolve a disagreement"
    );
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), stale_evidence());

    // AN UNREADABLE ANSWER searched nothing at all, so it cannot have found
    // the disagreement gone either. A five-second router timeout must not
    // rearm the warning.
    //
    // NO EVIDENCE ROWS: an unreachable router read no keys, and a key row
    // over nothing reads as a key that failed to load. The one row NAMES THE
    // TWO SETTINGS TO CHECK, because a rejected api_key reads exactly like an
    // unreachable router here and "unknown" alone leaves nowhere to go.
    router.set_listing(NO_LISTING);
    assert_eq!(
        home(),
        [concat!(
            "home: unknown: the router returned no readable client list, so nothing was established; ",
            "check router_url and api_key in [plugins.home_presence] ",
            "(a rejected key reads the same here as an unreachable router)"
        )]
    );
    assert!(
        memory().is_some(),
        "an unreadable answer does not resolve a disagreement"
    );
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), stale_evidence());
}

#[test]
fn a_state_directory_that_cannot_be_used_leaves_every_home_row_standing() {
    // THE MEMORY IS FAIL-QUIET: a state directory that is a regular FILE
    // breaks every read and every write of it, and the verdict, the evidence
    // and the warning must not notice.
    let sandbox = Sandbox::new("home-unusable-state");
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let blocked = sandbox.path("state-is-a-file");
    std::fs::write(&blocked, "not a directory\n").expect("a file where the state dir would go");
    let home = || {
        let mut probe = home_probe(&sandbox);
        probe.env("PNS_STATE_DIR", &blocked);
        home_rows(&stdout(&probe.output().expect("the engine runs")))
    };

    assert_eq!(home(), stale_evidence_warned());
    // The DOCUMENTED COST, pinned so it stays a cost and not a crash:
    // nothing could be remembered, so the same state is news again. A run
    // that went quiet here would mean a write had silently succeeded
    // somewhere this test cannot see.
    assert_eq!(home(), stale_evidence_warned());
    assert!(blocked.is_file(), "the blocking file is left as it was");
}

// --- the stale warning, delivered -------------------------------------------

#[test]
fn a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence() {
    // THE WARNING BECOMES A NOTIFICATION. The same condition that prints the
    // sentence hands it to the engine as an ordinary event, so the hand run
    // that prints the warning now delivers it too.
    let sandbox = Sandbox::new("home-stale-alert");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    assert_eq!(home_rows(&stdout(&output)), stale_evidence_warned());
    let delivered = alerts(&sandbox);
    assert_eq!(delivered.len(), 1, "one state, one alert: {delivered:?}");
    let alert = &delivered[0];
    // THE SAME SENTENCE the terminal printed, because both read it out of
    // `stale_warning`.
    assert_eq!(alert["detail"], STALE_WARNING);
    assert_eq!(alert["message"], STALE_WARNING);
    // An event ABOUT the reading, not from an agent: the title says which
    // subsystem and what happened.
    assert_eq!(alert["title"], "pns \u{b7} stale");
    assert_eq!(alert["agent"], "pns");
    assert_eq!(alert["state"], "stale");
    // No pane to focus, no project and no branch: nothing here came from a
    // terminal or a repository.
    assert_eq!(alert["pane"], "");
    assert_eq!(alert["project"], "");
    assert_eq!(alert["branch"], "");
}
