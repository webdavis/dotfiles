use super::*;

#[test]
fn the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it() {
    // HISTORY BELOW HISTORY, both below the gradeable pairing lines. An
    // unreplayed journal is not a failure, so the count sits under the one
    // section that already cannot move the exit code.
    let sandbox = Sandbox::new("doctor-journal-count");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    run(logged_event(&sandbox)
        .env("PNS_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "done"]));

    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines = report_rows(&printed);
    let heading = lines
        .iter()
        .position(|line| *line == format!("the last decision,{DECISION_HEADING_TAIL}"))
        .unwrap_or_else(|| panic!("no decision heading in {printed}"));
    assert_eq!(
        lines.last(),
        Some(&TWO_WAITING),
        "the count is the last line: {printed}"
    );
    assert_eq!(
        lines.len(),
        heading + 3,
        "one decision, then the count, and nothing else: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends and the pairing alone own the exit code: {}",
        stderr(&output)
    );
}

#[test]
fn a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code() {
    // ABSENT IS ITS OWN STATE with its own honest line. This is the OTHER one:
    // something is at the path and the read failed, which is a different thing
    // to say. A directory is the portable way to produce a real read error.
    let sandbox = Sandbox::new("doctor-journal-unreadable");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::create_dir_all(journal_path(&sandbox)).expect("a directory at the journal");

    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines = report_rows(&printed);
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("state import missed-notifications:")
    );
    let last = lines[lines.len() - 2];
    let opening = "the missed-notification journal could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NONE_WAITING),
        "and it is never told as an absent journal: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
}

#[test]
fn a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind() {
    // THE SAME PARK, on the file this slice added: the count is read on the
    // doctor's way out, so a FIFO here wedges the command after it has
    // already sent to every channel.
    let sandbox = Sandbox::new("doctor-journal-fifo");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let path = journal_path(&sandbox);
    plant_fifo(&path);

    let mut command = doctor_command(&sandbox);
    let output = output_before_the_deadline(&mut command);

    let printed = stdout(&output);
    let lines = report_rows(&printed);
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("state import missed-notifications:")
    );
    let last = lines[lines.len() - 2];
    let opening = "the missed-notification journal could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NONE_WAITING),
        "and it is never told as an absent journal: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
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
fn the_doctor_leaves_the_journal_exactly_as_it_found_it() {
    // READING IS ALLOWED, WRITING IS NOT. A doctor that journaled would file a
    // miss for the act of going to look for one, and its own test send is the
    // last event that should ever be replayed.
    let sandbox = Sandbox::new("doctor-journal-readonly");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::write(journal_path(&sandbox), planted_journal(2)).expect("the journal");
    let before = std::fs::read(journal_path(&sandbox)).expect("the journal");

    doctor_command(&sandbox).output().expect("the engine runs");

    assert_eq!(
        std::fs::read(journal_path(&sandbox)).expect("the journal"),
        before,
        "the doctor wrote to the journal it was reading"
    );
    assert_eq!(stored_records::text(&sandbox, "journal").as_bytes(), before);
    doctor_command(&sandbox)
        .output()
        .expect("the engine runs again");
    assert_eq!(stored_records::text(&sandbox, "journal").as_bytes(), before);
    assert!(decisions(&sandbox).is_empty(), "doctor recorded a decision");
    assert!(activity(&sandbox).is_empty(), "doctor recorded activity");
    assert!(
        stored_records::claims(&sandbox).is_empty(),
        "doctor claimed the journal"
    );
}
