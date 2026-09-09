use super::*;

#[test]
fn a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay() {
    // MEASURED ON THE RING and inherited by every reader of these files:
    // opening a FIFO BLOCKS until the other end is opened, for READING as
    // much as for writing, so a replay that trusted the path would park the
    // hook that called it. REFUSED RATHER THAN REPAIRED, which is what the
    // claim's own guard is for: a rename would move the operator's FIFO to
    // the claim path and the remove would then destroy it.
    let sandbox = Sandbox::new("replay-fifo");
    record_every_event(&sandbox);
    let path = journal_path(&sandbox);
    plant_fifo(&path);

    let mut command = present_event(&sandbox);
    let output = output_before_the_deadline(&mut command);

    assert_eq!(
        output.status.code(),
        Some(0),
        "a journal nobody could read costs the event nothing"
    );
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 1, "the live event alone: {raised:?}");
    assert_eq!(raised[0]["state"], "done", "{raised:?}");
    assert_eq!(stdout(&output), "", "nothing is said about the journal");
    // THE WHOLE STREAM, not a substring of it: a leaked error print uses the
    // operating system's own words, which share no predictable word with
    // anything this slice writes.
    assert_eq!(stderr(&output), "", "the event path gained stderr");
    assert!(
        std::fs::symlink_metadata(&path)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the journal's path was rewritten"
    );
}

#[test]
fn an_event_with_nothing_waiting_delivers_and_leaves_exactly_what_it_did_before() {
    // A MACHINE THAT NEVER MISSED ONE has no journal file at all, which is by
    // far the common case, and this slice must be invisible on it: the same
    // one notification, the same empty streams, the same exit, and nothing new
    // in the state directory.
    let sandbox = Sandbox::new("replay-nothing-waiting");
    record_every_event(&sandbox);

    let output = present_event(&sandbox).output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "", "the run gained a line");
    assert_eq!(stderr(&output), "", "the run gained stderr");
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "one notification, the live one: {raised:?}"
    );
    assert_eq!(raised[0]["state"], "done", "{raised:?}");
    stored_records::assert_consumed(&sandbox);
    assert_eq!(activity(&sandbox).len(), 1);
    assert_eq!(decisions(&sandbox).len(), 1);
    assert!(last_present(&sandbox).is_some());
}

#[test]
fn an_event_narrowed_to_no_channel_at_all_leaves_the_journal_where_it_found_it() {
    // Contradictory scope flags refuse before state access, leaving the
    // waiting journal unimported and unclaimed.
    let sandbox = Sandbox::new("replay-no-legs");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");
    let files_before = state_files(&sandbox);

    let output = run(present_event(&sandbox).args(["--local-only", "--remote-only"]));

    assert!(
        stdout(&output).contains("post SKIPPED"),
        "the contradiction really was reached: {}",
        stdout(&output)
    );
    assert!(
        events(&sandbox, "macos-banner").is_empty(),
        "nothing was delivered, replay included"
    );
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed with nowhere to send it"
    );
    assert_eq!(
        state_files(&sandbox),
        files_before,
        "the refusal initialized storage or claimed the queue"
    );
}

#[test]
fn a_queued_replay_releases_its_journal_after_attempts_and_preserves_it_on_interruption() {
    // The ledger owns later attempts before a completed handoff releases its
    // journal. Interruption leaves the same batch available for adoption.
    let delivered = Sandbox::new("replay-claim-delivered");
    record_every_event(&delivered);
    std::fs::write(journal_path(&delivered), planted_journal(2)).expect("the journal");
    run(&mut present_event(&delivered));
    assert_eq!(
        events(&delivered, "macos-banner").len(),
        2,
        "the replay really was delivered"
    );
    stored_records::assert_consumed(&delivered);

    // The banner hangs on the replay alone, so the live event is delivered
    // first and the process dies inside the catch-up dispatch. This unfinished
    // attempt must leave its held bytes for a later return to adopt.
    let killed = Sandbox::new("replay-claim-killed");
    record_every_event(&killed);
    killed.stub_channel(
        "macos-banner",
        &format!(
            "payload=$(cat)\nprintf '%s\\n' \"$payload\" >>\"{root}/macos-banner.events\"\n\
             case \"$payload\" in\n  *'\"state\":\"missed\"'*) : >\"{root}/inside.the.replay\"; \
             for _ in $(seq 1 1000); do [ -e \"{root}/the.test.is.over\" ] && break; sleep 0.01; done; : >\"{root}/channel.finished\" ;;\nesac",
            root = killed.display()
        ),
    );
    std::fs::write(journal_path(&killed), planted_journal(2)).expect("the journal");
    let mut command = present_event(&killed);
    // The guard releases the inert descendant on assertion failure too. Files
    // keep an inherited pipe from holding the fixture open after interruption.
    struct Interrupted {
        child: std::process::Child,
        release: std::path::PathBuf,
    }
    impl Drop for Interrupted {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::write(&self.release, "");
        }
    }
    let child = command
        .stdout(std::fs::File::create(killed.path("child.stdout")).expect("stdout"))
        .stderr(std::fs::File::create(killed.path("child.stderr")).expect("stderr"))
        .spawn()
        .expect("the engine starts");
    let mut child = Interrupted {
        child,
        release: killed.path("the.test.is.over"),
    };
    let inside = killed.path("inside.the.replay");
    // PATIENCE, NOT A MEASUREMENT. Nothing here claims the replay is fast; the
    // wait exists so a hung engine fails with its stderr rather than hanging the
    // suite. The old 650 ms was tight enough that a full parallel run, where a
    // process spawn and a shell stub compete with every other test on this
    // machine, tripped it on work that was proceeding normally. The stub's own
    // hold loop is generous for the same reason, and both release early: this
    // one the moment the marker appears, the stub the moment the guard writes
    // its release file.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !inside.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the replay never reached a channel: {}",
            std::fs::read_to_string(killed.path("child.stderr")).unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let held_before = stored_records::claims(&killed);
    assert_eq!(
        held_before.len(),
        2,
        "both journal rows are held during delivery"
    );
    assert!(
        held_before
            .iter()
            .all(|(_, owner, _)| *owner == child.child.id())
    );
    let queued: i64 = database(&killed)
        .query_row(
            "SELECT count(*) FROM return_claims c JOIN ledger_events e
             ON e.producer='pns-return' AND e.request_id=c.request_id
             JOIN ledger_legs l ON l.event=e.seq
             JOIN ledger_attempts a ON a.leg=l.id AND a.generation=l.generation
             WHERE c.owner=?1 AND e.state='missed' AND l.destination='macos-banner'
             AND l.acknowledged=0 AND a.finished IS NULL AND a.outcome=0",
            [child.child.id()],
            |row| row.get(0),
        )
        .expect("the actual replay ownership record");
    assert_eq!(
        queued, 1,
        "the ledger must own this exact held replay before dispatch"
    );
    let _ = child.child.kill();
    let _ = child.child.wait();
    // The process guardian kills the channel after producer death. The release
    // also ends the inert polling loop if it observes the marker first.
    std::fs::write(killed.path("the.test.is.over"), "").expect("the release");
    // AND THE MARKER IS ALREADY BACK, which is the OTHER half of what this
    // process was killed to prove. The window's near edge is restored inside
    // the claim, before anything is counted and long before anything is
    // dispatched. A run killed mid-delivery leaves its held batch for recovery
    // without losing the next window. A build that restored the edge after
    // the dispatch leaves no marker here at all, and the window it consumed
    // could never fire again.
    let held = stored_records::claims(&killed);
    assert_eq!(
        held, held_before,
        "the interrupted replay lost or reassigned its held rows"
    );
    assert_eq!(
        held.iter()
            .map(|(_, _, line)| line.as_str())
            .collect::<String>(),
        planted_journal(2)
    );
    assert!(
        journal(&killed).is_empty(),
        "claimed rows remain owned rather than pending"
    );
    assert!(
        last_present(&killed).is_some_and(|edge| edge > 1_700_000_000),
        "the restored edge is this event's own clock read: {:?}",
        last_present(&killed)
    );
}
