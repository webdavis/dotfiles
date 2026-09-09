use super::*;

#[test]
fn the_shared_append_prunes_each_ring_to_its_own_callers_depth() {
    // ONE HELPER, TWO DEPTHS, which is exactly where an off-by-one hides. Both
    // files start AT their caps, so the one event below pushes each of them
    // over by exactly one and the prune has to answer with a different number
    // for each. A journal silently pruning to the ring's five fails here.
    let sandbox = Sandbox::new("journal-two-depths");
    mute(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(JOURNAL_KEPT)).expect("the journal");
    let ring: String = (0..RING_KEPT)
        .map(|which| format!("1756499000 c{which}/done surface=Away\n"))
        .collect();
    std::fs::write(sandbox.path("state/decisions"), ring).expect("the ring");

    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--detail", "this event's own summary"]));

    let waiting = journal(&sandbox);
    // THE APPEND REALLY RAN, asserted before the count: a journal nothing
    // wrote is still exactly its planted depth, so the count alone would pass
    // on a build that never journals anything at all.
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "this event's own summary",
        "the newest entry is this event's: {waiting:?}"
    );
    assert_eq!(
        waiting.len(),
        JOURNAL_KEPT,
        "the journal kept its own depth"
    );
    assert_eq!(
        decisions(&sandbox).len(),
        RING_KEPT,
        "and the ring kept its own"
    );
}

#[test]
fn a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown() {
    // THE MUTE'S QUEUE. The operator muted, so the matrix's card never fired
    // and nothing reached them, while the durable log still has the event in
    // full. What lands here is the minimum a replay needs to rebuild the card.
    let sandbox = Sandbox::new("journal-append");
    mute(&sandbox);
    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "blocked"])
        .args(["--project", "dotfiles", "--branch", "main"])
        .args(["--detail", "a private summary"]));
    assert!(
        sandbox.fired("hermes"),
        "the durable log is exempt from the mute and still has the event in full"
    );
    assert!(!sandbox.fired("mobile"), "and the card the mute swallowed");

    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), 1, "exactly one entry: {waiting:?}");
    for (name, expected) in [
        ("agent", "claude"),
        ("state", "blocked"),
        ("project", "dotfiles"),
        ("branch", "main"),
        ("detail", "a private summary"),
    ] {
        assert_eq!(field(&waiting[0], name), expected, "{name}: {waiting:?}");
    }
    let parsed: serde_json::Value = serde_json::from_str(&waiting[0]).expect("one JSON object");
    assert!(
        parsed["at"].as_u64().is_some_and(|at| at > 1_700_000_000),
        "the decision's own clock read: {waiting:?}"
    );
}

#[test]
fn a_delivered_event_journals_nothing_at_all() {
    // NO FILE ON A MACHINE THAT NEVER MISSED ONE, which is what makes the
    // journal's presence meaningful. The native banner acknowledges its send,
    // so there is nothing to replay.
    let sandbox = Sandbox::new("journal-delivered");
    run(acknowledged_banner(&sandbox)
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        sandbox.path("notifier.args").exists(),
        "the native banner ran"
    );
    assert!(
        journal(&sandbox).is_empty(),
        "a delivered event left a journal behind"
    );
}

#[test]
fn the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone() {
    // THE FILE IS WHAT IS WAITING, never everything that was ever missed. The
    // planted journal starts AT the cap, so this event pushes it over by
    // exactly one and the oldest entry is the one that has to go.
    let sandbox = Sandbox::new("journal-prune");
    mute(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(JOURNAL_KEPT)).expect("the journal");

    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--detail", "the newest miss"]));

    let waiting = journal(&sandbox);
    assert_eq!(waiting.len(), JOURNAL_KEPT, "got {} entries", waiting.len());
    assert_eq!(
        field(&waiting[0], "detail"),
        "planted 1",
        "the oldest was dropped: {waiting:?}"
    );
    assert_eq!(
        field(waiting.last().expect("a journal"), "detail"),
        "the newest miss",
        "and the newest is last: {waiting:?}"
    );
}

#[test]
fn a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event() {
    // MEASURED ON THE RING and inherited here by sharing its append: opening a
    // FIFO for writing BLOCKS until something opens the read end, so an append
    // that trusted the path would park the hook that called it, on every
    // event, until the machine is rebooted.
    let sandbox = Sandbox::new("journal-fifo");
    mute(&sandbox);
    let path = journal_path(&sandbox);
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );

    let output = output_before_the_deadline(
        logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]),
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a journal nobody could write costs the event nothing"
    );
    assert!(sandbox.fired("hermes"), "the durable log still fired");
    assert_eq!(stdout(&output), "", "nothing is said about the journal");
    // THE WHOLE STREAM, not a substring of it. A leaked `eprintln!("{error}")`
    // prints the operating system's own words, which share no predictable word
    // with "missed", so only an empty stream is evidence that the drop really
    // is a drop.
    assert_eq!(stderr(&output), "", "the event path gained stderr");
    // REFUSED, NOT REPAIRED: the path still holds what it held.
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the journal's path was rewritten"
    );
}

#[test]
fn a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing() {
    // FAIL-QUIET, in `record_decision`'s style. A journal entry that did not
    // land costs a replay, never a card, and a complaint printed here would
    // put a line about the state directory into every hook's output for the
    // rest of this machine's life.
    let sandbox = Sandbox::new("journal-unwritable");
    mute(&sandbox);
    // Import the mute first, then reject the journal INSERT immediately. This
    // isolates publication failure from unreadable-mute policy and busy waits.
    pns_adapters::SqliteStore::for_records(sandbox.path("state"))
        .quiet_expiry()
        .unwrap();
    let writer = rusqlite::Connection::open_with_flags(
        sandbox.path("state/pns.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .unwrap();
    writer
        .execute_batch(
            "CREATE TRIGGER refuse_journal BEFORE INSERT ON journal
             BEGIN SELECT RAISE(ABORT, 'owned journal refusal'); END;",
        )
        .unwrap();
    let output = logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .output()
        .expect("the engine runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(sandbox.fired("hermes"), "every channel still fires");
    assert_eq!(stdout(&output), "", "nothing is said about the write");
    // THE WHOLE STREAM, for the reason the FIFO's test states: an unwritable
    // state directory fails the decision record and the journal entry alike,
    // and neither complaint would have to contain either word to be a
    // complaint.
    assert_eq!(stderr(&output), "", "the event path gained stderr");
    assert!(journal(&sandbox).is_empty(), "and nothing was written");
}

#[test]
fn the_journal_is_created_readable_and_writable_by_its_owner_alone() {
    // THE MODE ITSELF IS THE ASSERTION, not "narrower than the umask": the
    // file holds the operator's own text and nothing in the state directory
    // has a reason to be world-readable.
    let sandbox = Sandbox::new("journal-mode");
    mute(&sandbox);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert_eq!(journal_mode(&sandbox), 0o600, "the append created it");

    // AND AFTER A PRUNE, which is a SECOND create: the prune publishes by
    // renaming a pending file over the journal, so the pending file's mode is
    // the one the journal ends up wearing.
    let store = pns_adapters::SqliteStore::for_records(sandbox.path("state"));
    for which in 0..JOURNAL_KEPT {
        store
            .record_journal(
                &pns_domain::EventArgs {
                    agent: "claude".into(),
                    state: "done".into(),
                    detail: format!("planted {which}"),
                    ..Default::default()
                },
                Some(1_756_499_000),
                None,
            )
            .expect("seed through the real journal writer");
    }
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done", "--detail", "y"]));
    assert_eq!(
        journal(&sandbox).len(),
        JOURNAL_KEPT,
        "the prune really ran"
    );
    assert_eq!(journal_mode(&sandbox), 0o600, "the prune republished it");
}
