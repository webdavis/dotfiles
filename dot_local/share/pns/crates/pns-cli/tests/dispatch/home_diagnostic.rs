use super::*;

#[test]
fn the_home_diagnostic_always_shows_the_evidence_and_warns_once_per_stale_state() {
    // THE MEMORY IS DURABLE under a real HOME, which is the one edge this
    // slice adds: the dedupe is only worth anything if a LATER run of the
    // binary reads what this one wrote.
    let sandbox = Sandbox::new("home-staleness");
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    // RAW STDOUT, trailing newline and all: "the diagnostic is unchanged" is a
    // claim about the BYTES, and a trim would hide a blank line growing on the
    // end of every line this file pins.
    let home = || {
        let mut probe = home_probe(&sandbox);
        stdout(&run(&mut probe)).to_string()
    };
    let evidence = STALE_EVIDENCE;
    let warning = STALE_WARNING;
    let memory = || {
        use rusqlite::OptionalExtension;
        stored_records::at(&sandbox.path(".local/state/pns/pns.db"))
            .query_row("SELECT body FROM staleness WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .expect("the stored episode")
    };

    assert_eq!(home(), format!("{evidence}\n{warning}\n"));
    assert_eq!(
        memory().expect("the episode is remembered").trim(),
        "device_mac device_hostname=none device_ipv4=other"
    );
    // A REPEAT still tells the whole truth and says the warning no more.
    assert_eq!(home(), format!("{evidence}\n"));
    // RESOLVED: the memory is forgotten rather than left to suppress the next
    // episode, and the state is news again when it comes back.
    router.set_listing(KEYS_AGREE);
    assert_eq!(
        home(),
        "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")\n\
         home:   device_mac \"2e:11:ab:6d:b0:4f\" matched the client the verdict names\n\
         home:   device_hostname \"mister-2\" matched the client the verdict names\n\
         home:   device_ipv4 \"192.168.1.248\" matched the client the verdict names\n"
    );
    assert!(memory().is_none(), "a resolved episode is forgotten");
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), format!("{evidence}\n{warning}\n"));

    // AWAY IS NOT RESOLVED. Leaving the house says nothing about the
    // identifiers: every key matches nothing because the device is not on the
    // wifi, so the live episode survives the trip and the homecoming is quiet.
    // Without this the warning is once per HOMECOMING, which for a phone is
    // once a day.
    router.set_listing(KEYS_AWAY);
    assert_eq!(
        home(),
        "home: NOT on the home network (no configured identifier matched a client)\n\
         home:   device_mac \"2e:11:ab:6d:b0:4f\" matched no client\n\
         home:   device_hostname \"mister-2\" matched no client\n\
         home:   device_ipv4 \"192.168.1.248\" matched no client\n"
    );
    assert!(
        memory().is_some(),
        "leaving the house does not resolve a disagreement"
    );
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), format!("{evidence}\n"));

    // AN UNREADABLE ANSWER searched nothing at all, so it cannot have found
    // the disagreement gone either. A five-second router timeout must not
    // rearm the warning.
    router.set_listing(NO_LISTING);
    assert_eq!(
        home(),
        "home: unknown (router unreachable or its answer unreadable)\n"
    );
    assert!(
        memory().is_some(),
        "an unreadable answer does not resolve a disagreement"
    );
    router.set_listing(KEYS_DISAGREE);
    assert_eq!(home(), format!("{evidence}\n"));
}

#[test]
fn a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing() {
    // THE MEMORY IS THIS SLICE'S ONE EDGE and it is FAIL-QUIET: a state
    // directory that is a regular FILE breaks every read and every write of
    // it, and the verdict, the evidence, the warning and the exit code must
    // not notice. `run` asserts the exit 0, which is the half a stray
    // `unwrap` would take out.
    let sandbox = Sandbox::new("home-unusable-state");
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let blocked = sandbox.path("state-is-a-file");
    std::fs::write(&blocked, "not a directory\n").expect("a file where the state dir would go");
    let home = || {
        let mut probe = home_probe(&sandbox);
        probe.env("PNS_STATE_DIR", &blocked);
        stdout(&run(&mut probe)).to_string()
    };

    assert_eq!(home(), format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n"));
    // The DOCUMENTED COST, pinned so it stays a cost and not a crash:
    // nothing could be remembered, so the same state is news again. A run
    // that went quiet here would mean a write had silently succeeded
    // somewhere this test cannot see.
    assert_eq!(home(), format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n"));
    assert!(blocked.is_file(), "the blocking file is left as it was");
}

// --- the stale warning, delivered -------------------------------------------

#[test]
fn a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence() {
    // THE WARNING BECOMES A NOTIFICATION. The same condition that prints the
    // sentence hands it to the engine as an ordinary event, so the hand run
    // that prints the warning now delivers it too. NOTHING SCHEDULES `pns
    // home` yet, so a stale identifier still waits for someone to type the
    // command; what this closes is the reach past that one terminal, and the
    // scheduling is later work.
    let sandbox = Sandbox::new("home-stale-alert");
    count_alerts(&sandbox);
    let router = RouterStub::start(KEYS_DISAGREE);
    sandbox.write_config(&stale_config(&router.url()));
    let mut probe = home_probe(&sandbox);
    let output = run(&mut probe);

    // THE DIAGNOSTIC IS UNCHANGED, byte for byte: it grew a consumer, not a
    // new way of saying things. RAW, so "byte for byte" means it, down to the
    // single newline `println!` ends the report with.
    assert_eq!(
        stdout(&output),
        format!("{STALE_EVIDENCE}\n{STALE_WARNING}\n")
    );
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
