use super::*;

#[test]
fn an_event_appends_exactly_one_decision_carrying_what_it_decided_and_what_the_legs_did() {
    // THE RECORD IS WRITTEN AFTER DISPATCH, so the leg verdicts are part of
    // it: without them the log says pns decided to card the operator while
    // their question is why no card appeared.
    let sandbox = Sandbox::new("decision-log-append");
    run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done", "--long-running"])
        .args(["--project", "dotfiles", "--detail", "a private summary"]));
    assert!(sandbox.fired("mobile"), "the channels fired");

    let recorded = decisions(&sandbox);
    assert_eq!(recorded.len(), 1, "exactly one line: {recorded:?}");
    let entry = &recorded[0];
    for expected in [
        " claude/done ",
        " surface=Away ",
        " long_running=yes ",
        " pane=none ",
        " plan=banner:no,card:yes,pulse:yes ",
        " legs=mobile:silent,hermes:silent",
    ] {
        assert!(
            entry.contains(expected),
            "{expected:?} missing from {entry}"
        );
    }
    for content in ["a private summary", "dotfiles"] {
        assert!(!entry.contains(content), "free text reached {entry}");
    }
}

#[test]
fn an_event_that_reached_no_channel_at_all_still_records_its_decision() {
    // THE CASE THE LOG EXISTS FOR. "Nothing fired" is exactly what an operator
    // opens the report to ask about, so the empty-plan branch records too.
    let sandbox = Sandbox::new("decision-log-empty-plan");
    let output = run(logged_event(&sandbox)
        .args(["--agent", "claude", "--state", "done"])
        .args(["--local-only", "--remote-only"]));
    assert!(!sandbox.fired("hermes"), "both flags suppress everything");

    let recorded = decisions(&sandbox);
    assert_eq!(recorded.len(), 1, "got {recorded:?}");
    for expected in [" local_only=yes ", " remote_only=yes ", " legs=none"] {
        assert!(
            recorded[0].contains(expected),
            "{expected:?} missing from {}",
            recorded[0]
        );
    }
    assert!(
        stdout(&output).contains("post SKIPPED"),
        "and the contradiction is still said out loud"
    );
}

#[test]
fn the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone() {
    // A SINGLE SLOT DOES NOT SURVIVE BEING LOOKED AT: the Stop hook of the
    // session the operator is typing `pns doctor` into fires its own event.
    //
    // CHECKED AFTER EVERY EVENT, not only at the end. The prune runs only when
    // the file went over the cap, so a cap wrong by one settles back into a
    // correct-looking ring one event later: measured, a ring keeping four was
    // indistinguishable from a ring keeping five by the seventh turn.
    let sandbox = Sandbox::new("decision-log-ring");
    let cap = 5;
    for turn in 1..=7 {
        run(logged_event(&sandbox).args(["--agent", &format!("c{turn}"), "--state", "done"]));
        let recorded = decisions(&sandbox);
        assert_eq!(
            recorded.len(),
            turn.min(cap),
            "after turn {turn}: {recorded:?}"
        );
        let oldest = turn.saturating_sub(cap) + 1;
        assert!(
            recorded[0].contains(&format!(" c{oldest}/done ")),
            "after turn {turn} the oldest kept should be c{oldest}: {recorded:?}"
        );
        assert!(
            recorded[recorded.len() - 1].contains(&format!(" c{turn}/done ")),
            "after turn {turn} the newest should be last: {recorded:?}"
        );
    }
}

#[test]
fn a_state_directory_that_cannot_be_written_costs_the_event_nothing() {
    // FAIL-QUIET, in `remember_staleness`'s style. A decision that did not
    // record is a diagnostic missing later; a complaint printed here would put
    // a line about the state directory into every hook's output for the rest
    // of this machine's life. `run` asserts the exit 0.
    let sandbox = Sandbox::new("decision-log-unwritable");
    let blocked = sandbox.path("state-is-a-file");
    std::fs::write(&blocked, "not a directory\n").expect("a file where the state dir would go");
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", &blocked);
    let output = run(command.args(["--agent", "claude", "--state", "done"]));

    assert!(sandbox.fired("mobile"), "every channel still fires");
    assert!(sandbox.fired("hermes"));
    assert_eq!(stdout(&output), "", "nothing is said about the write");
    assert!(
        !stderr(&output).contains("decision"),
        "nor on the other stream: {}",
        stderr(&output)
    );
}

#[test]
fn a_fifo_at_the_rings_path_is_never_opened_and_never_parks_the_event() {
    // MEASURED: opening a FIFO for writing BLOCKS until something opens the
    // read end, so an append that trusts the path parks the hook that called
    // it, on every event, until the machine is rebooted. The ring is state
    // this tool owns; a FIFO is not that state, and nothing that is not a
    // regular file is opened at all.
    let sandbox = Sandbox::new("decision-log-fifo");
    let ring = ring_path(&sandbox);
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&ring)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );

    let status = run_before_the_deadline(
        logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]),
    );
    assert_eq!(
        status.code(),
        Some(0),
        "a record nobody could write costs the event nothing"
    );
    for channel in ["mobile", "hermes"] {
        assert!(sandbox.fired(channel), "{channel} never fired");
    }
    // REFUSED, NOT REPAIRED: the path still holds what it held. Healing an
    // irregular file would mean this tool deleting something it did not put
    // there, on a path it only ever appends to.
    assert!(
        std::fs::symlink_metadata(&ring)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the ring's path was rewritten"
    );
}

#[test]
fn a_ring_holding_bytes_that_are_not_text_heals_to_a_bounded_readable_one() {
    // MEASURED: the read-back is what the prune runs on, so one byte no
    // reader can decode does not cost one entry, it switches the prune OFF
    // and the ring then grows without a bound for the rest of the machine's
    // life. The corrupt prefix is foreign and may go; what has to survive is
    // this event's own line, on a file the next append can use.
    let sandbox = Sandbox::new("decision-log-not-text");
    let ring = ring_path(&sandbox);
    std::fs::write(&ring, b"\xff\xfe not a decision\n").expect("the corrupt ring");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let healed = stored_records::text(&sandbox, "decisions");
    assert_eq!(healed.lines().count(), 1, "got {healed:?}");
    assert!(healed.contains(" claude/done "), "got {healed:?}");

    // AND THE HEAL LEAVES AN ORDINARY RING: the next event appends to it
    // rather than healing a second time.
    run(logged_event(&sandbox).args(["--agent", "codex", "--state", "done"]));
    let after = stored_records::text(&sandbox, "decisions");
    assert_eq!(after.lines().count(), 2, "got {after:?}");
    assert!(
        after.contains(" claude/done ") && after.contains(" codex/done "),
        "got {after:?}"
    );
}

#[test]
fn a_ring_that_ends_mid_line_never_fuses_the_next_record_onto_it() {
    // MEASURED: an append lands at the last byte, so a file left without its
    // trailing newline (a truncated write, a hand edit) WELDS the new record
    // onto the tail of the old one, and the reader then cannot read either.
    let sandbox = Sandbox::new("decision-log-no-newline");
    let ring = ring_path(&sandbox);
    std::fs::write(&ring, "1756499000 a/one surface=Desk").expect("the truncated ring");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let contents = stored_records::text(&sandbox, "decisions");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "got {lines:?}");
    assert_eq!(
        lines[0], "1756499000 a/one surface=Desk",
        "the entry that was already there is left as it was"
    );
    assert!(
        lines[1].starts_with(char::is_numeric) && lines[1].contains(" claude/done "),
        "the new record has its own line and its own epoch: {lines:?}"
    );
}

#[test]
fn a_ring_too_large_to_read_back_is_replaced_rather_than_slurped() {
    // MEASURED: the read-back is unbounded, so whatever sits at the path is
    // pulled into memory whole on every event. A file with no line breaks in
    // it also prunes to nothing, so once one is there it stays there.
    let sandbox = Sandbox::new("decision-log-oversize");
    let ring = ring_path(&sandbox);
    let bloated = format!("{}\n", "z".repeat(400_000));
    std::fs::write(&ring, &bloated).expect("the bloated ring");

    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let healed = stored_records::text(&sandbox, "decisions");
    assert!(
        healed.len() < bloated.len() / 100,
        "the ring is still {} bytes",
        healed.len()
    );
    assert_eq!(
        healed.lines().count(),
        1,
        "healed to this event's line alone: {healed:?}"
    );
    assert!(healed.contains(" claude/done "), "got {healed:?}");
}

#[test]
fn events_racing_each_other_lose_no_line_and_leave_no_pending_file() {
    // WRITTEN BY APPEND, never read-modify-write. A Stop hook and the
    // long-running notifier firing together is an ordinary pair, and a ring
    // rewritten from a read taken before the other event's line landed drops
    // that line. FEWER THAN THE CAP ON PURPOSE, so no prune runs at all and
    // the count is exact rather than a range: every one of these has to be
    // there, whole, with nothing half-published beside it.
    let sandbox = Sandbox::new("decision-log-concurrent");
    ring_path(&sandbox);
    let racing: Vec<std::process::Child> = (1..=5)
        .map(|turn| {
            logged_event(&sandbox)
                .args(["--agent", &format!("c{turn}"), "--state", "done"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("the engine starts")
        })
        .collect();
    for mut child in racing {
        assert!(
            child.wait().expect("the child is waitable").success(),
            "every racing event exits 0"
        );
    }

    let contents = stored_records::text(&sandbox, "decisions");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 5, "a line was lost: {lines:?}");
    for turn in 1..=5 {
        assert!(
            contents.contains(&format!(" c{turn}/done ")),
            "c{turn} is missing: {lines:?}"
        );
    }
    for entry in &lines {
        assert!(
            entry.starts_with(char::is_numeric) && entry.contains(" legs="),
            "a torn or fused line survived the race: {entry:?}"
        );
    }
    let pending: Vec<String> = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("decisions.new"))
        .collect();
    assert!(pending.is_empty(), "a publish left {pending:?} behind");
}
