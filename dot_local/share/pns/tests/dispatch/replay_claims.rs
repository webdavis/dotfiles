use super::*;

#[test]
fn a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed() {
    // MEASURED ON THE SHIPPED BUILD: the claim was removed before the read was
    // known to have worked, so a journal carrying one undecodable byte came
    // back as an empty batch and the whole queue was gone. Nothing delivered,
    // nothing left, nothing said.
    //
    // AN UNDELIVERED BATCH IS NEVER DESTROYED. What this run cannot read it
    // leaves exactly as it is, under a claim name the NEXT return adopts.
    let sandbox = Sandbox::new("replay-unreadable");
    record_every_event(&sandbox);
    let mut waiting = planted_journal(2).into_bytes();
    // A BYTE NO READER CAN DECODE, which is what the guarded reader refuses:
    // the journal is a plain file a backup tool or a hand edit can reach.
    waiting.extend_from_slice(b"\xff\xfe not text\n");
    std::fs::write(journal_path(&sandbox), &waiting).expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "a batch nobody could read was delivered anyway: {raised:?}"
    );
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("retained unreadable legacy journal"),
        waiting
    );
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "unreadable bytes were claimed as an empty batch"
    );
    let error: String = database(&sandbox)
        .query_row(
            "SELECT error FROM legacy_imports WHERE family = 'missed-notifications'",
            [],
            |row| row.get(0),
        )
        .expect("an unreadable import is recorded");
    assert_eq!(error, "legacy record could not be read");
}

#[test]
fn a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return() {
    // THE STRANDED CLAIM. A run killed between the rename and the delivery,
    // and a run that could not read what it claimed, both leave a claim file
    // behind; before this nothing ever looked at one again, so the queue sat
    // in the state directory for good and the doctor's count could not even
    // see it, because that count reads the journal's own name.
    let sandbox = Sandbox::new("replay-adopts-a-claim");
    record_every_event(&sandbox);
    let stranded = journal_path(&sandbox).with_extension("claim.999999");
    std::fs::write(&stranded, planted_journal(2)).expect("the stranded claim");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the stranded batch never reached the operator: {raised:?}"
    );
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "both stranded entries were adopted: {body}"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn a_held_batch_whose_owner_is_still_running_is_left_exactly_where_it_is() {
    // A HELD FILE IS A BATCH SOMEBODY IS READING RIGHT NOW, which is the whole
    // reason its name sits outside the prefix the adoption scan matches: take
    // one from a live owner and the double delivery the hold exists to prevent
    // is back with an extra step in front of it.
    //
    // TWO KINDS OF LIVE OWNER, because `kill(pid, 0)` has two ways of saying
    // the process is there. This test's own process answers success. Pid 1 is
    // launchd, which this user may not signal, and answers EPERM: an error,
    // and still a process that exists, so only ESRCH may count as gone.
    let sandbox = Sandbox::new("replay-live-hold");
    record_every_event(&sandbox);
    let mine = journal_path(&sandbox).with_extension(format!("held.{}", std::process::id()));
    let unsignalable = journal_path(&sandbox).with_extension("held.1");
    std::fs::write(&mine, planted_journal(2)).expect("the live hold");
    std::fs::write(&unsignalable, planted_journal(2)).expect("the unsignalable hold");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "a batch was taken from a process still holding it: {raised:?}"
    );
    assert_eq!(
        std::fs::read(&mine).expect("the live hold"),
        planted_journal(2).into_bytes(),
        "the batch its owner is still reading was touched"
    );
    assert_eq!(
        std::fs::read(&unsignalable).expect("the unsignalable hold"),
        planted_journal(2).into_bytes(),
        "a hold whose owner answers EPERM rather than ESRCH was read as gone"
    );
}

#[test]
fn a_held_batch_whose_owner_is_gone_is_adopted_exactly_once() {
    // THE OTHER HALF OF THE HOLD. A run killed between the rename that takes a
    // claim and the delivery leaves the batch under its own held name, and
    // that name is outside the claim prefix, so widening the scan to reach it
    // is what keeps the hold from being a way to lose a queue for good.
    let sandbox = Sandbox::new("replay-abandoned-hold");
    record_every_event(&sandbox);
    let abandoned = journal_path(&sandbox).with_extension("held.999999");
    std::fs::write(&abandoned, planted_journal(2)).expect("the abandoned hold");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the abandoned batch never came back: {raised:?}"
    );
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "both entries of the abandoned hold came back: {body}"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn an_unreadable_old_claim_cannot_starve_the_good_batch_behind_it() {
    // THE HELD NAME IS PER CLAIM (pid then a sequence), and this is why. With
    // one name per PROCESS, an unreadable first claim occupied it, every later
    // claim in the run deferred, and every FOLLOWING run's adoption migrated
    // the unreadable hold to its own fresh name first, so it always sorted
    // oldest and the good batch behind it starved forever. Here both are
    // handled in ONE run: the good batch delivers, the unreadable one parks.
    let sandbox = Sandbox::new("replay-no-starvation");
    record_every_event(&sandbox);
    let unreadable = journal_path(&sandbox).with_extension("claim.222");
    std::fs::write(&unreadable, b"\xff\xfe not text\n").expect("the unreadable claim");
    let good = journal_path(&sandbox).with_extension("claim.333");
    std::fs::write(&good, planted_journal(2)).expect("the good claim");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        2,
        "the good batch was starved by the unreadable one ahead of it: {raised:?}"
    );
    assert_eq!(raised[1]["state"], "missed", "{raised:?}");
    assert_eq!(
        std::fs::read(&unreadable).expect("retained unreadable claim"),
        b"\xff\xfe not text\n"
    );
    assert_eq!(
        std::fs::read_to_string(&good).expect("retained imported claim"),
        planted_journal(2)
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn a_hand_planted_negative_hold_name_is_never_read_as_a_pid() {
    // kill() reads a non-positive value as the GROUP and BROADCAST forms, so a
    // file named held.-99999 would probe process GROUP 99999 and, absent, read
    // as an abandoned hold. The parse refuses non-positive owners outright:
    // the file is left exactly where it was found, delivered by nobody.
    let sandbox = Sandbox::new("replay-negative-hold");
    record_every_event(&sandbox);
    let planted = journal_path(&sandbox).with_extension("held.-99999");
    std::fs::write(&planted, planted_journal(2)).expect("the planted hold");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(
        raised.len(),
        1,
        "a negative hold name was adopted through the group form: {raised:?}"
    );
    assert_eq!(
        std::fs::read(&planted).expect("the planted hold"),
        planted_journal(2).into_bytes(),
        "the planted hold was touched"
    );
}

#[test]
fn a_line_nothing_can_parse_costs_the_entries_around_it_nothing() {
    // A READABLE JOURNAL WITH A TORN LINE IN IT is not the same thing as a
    // claim nobody can read, and the two must not be answered the same way:
    // the append's own heal can republish a single line over this file, and
    // one line nobody can parse must not cost the notifications around it.
    let sandbox = Sandbox::new("replay-torn-line");
    record_every_event(&sandbox);
    std::fs::write(
        journal_path(&sandbox),
        format!("{}{{\"agent\":\"claude\",\n", planted_journal(2)),
    )
    .expect("the journal");

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 2, "{raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        body.starts_with("2 missed notifications. "),
        "the torn line is not an entry and is not counted: {body}"
    );
    stored_records::assert_consumed(&sandbox);
}

#[test]
fn a_present_event_narrowed_to_the_log_leaves_the_queue_for_a_surface_that_shows_it() {
    // MEASURED: `--remote-only` keeps the durable log alone, and a log is not
    // a surface anyone is looking at. The queue was claimed, posted into a log
    // that already holds every one of those events in full, and deleted, with
    // nothing the operator would ever see. A REPLAY NEEDS A DECORATIVE LEG.
    let sandbox = Sandbox::new("replay-remote-only");
    record_every_event(&sandbox);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(present_event(&sandbox).arg("--remote-only"));

    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "the live event alone reached the log: {logged:?}"
    );
    assert_eq!(logged[0]["state"], "done", "{logged:?}");
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed by a run that could show none of it"
    );
    assert_eq!(stored_records::text(&sandbox, "journal").as_bytes(), before);
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "the queue was claimed"
    );
}

#[test]
fn a_machine_with_only_a_durable_channel_never_consumes_the_queue_it_cannot_show() {
    // THE SAME HOLE WITH NO FLAG TYPED. A machine whose config enables hermes
    // alone still plans the banner the matrix asked for, and has nothing to
    // raise it with, so every present event would quietly eat the queue.
    let sandbox = Sandbox::new("replay-durable-only");
    record_every_event(&sandbox);
    sandbox.write_config("[plugins.hermes]\nenabled = true\n");
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    run(&mut present_event(&sandbox));

    let logged = events(&sandbox, "hermes");
    assert_eq!(
        logged.len(),
        1,
        "the live event alone reached the log: {logged:?}"
    );
    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the queue was consumed on a machine with no surface to show it on"
    );
    assert_eq!(stored_records::text(&sandbox, "journal").as_bytes(), before);
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "the queue was claimed"
    );
}

#[test]
fn a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found() {
    // THE CLAIM'S GUARD IS THE RENAME ITSELF. A check taken BEFORE it is a
    // check of a path something else is still free to change, so what the
    // rename actually claimed is verified AFTER it lands: anything that is not
    // a regular file goes back to the journal's own path untouched, and this
    // run declines rather than removing something it never wrote.
    let sandbox = Sandbox::new("replay-directory");
    record_every_event(&sandbox);
    // A MARKER INSIDE IT, so the assertion is that THIS directory came back
    // rather than that something directory-shaped is at the path.
    let planted = journal_path(&sandbox).join("not-a-journal");
    std::fs::create_dir_all(&planted).expect("a directory at the journal's path");

    run(&mut present_event(&sandbox));

    assert!(
        planted.is_dir(),
        "the directory never came back: {:?}",
        state_files(&sandbox)
    );
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "the directory became a claim"
    );
    let raised = events(&sandbox, "macos-banner");
    assert_eq!(raised.len(), 1, "the live event alone: {raised:?}");
}
