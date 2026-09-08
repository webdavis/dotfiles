use super::*;

#[test]
fn the_doctor_prints_the_decision_section_after_its_summary_newest_first() {
    // AFTER THE SUMMARY, not before it: the census plus its summary is one
    // complete thought whose line order is already pinned above, and appending
    // cannot disturb it.
    let sandbox = Sandbox::new("doctor-decision-section");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    for turn in 1..=2 {
        run(logged_event(&sandbox).args(["--agent", &format!("c{turn}"), "--state", "done"]));
    }
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    // ANCHORED ON THE HEADING THIS LOCATES ITSELF, rather than on an offset
    // from the summary. Every assertion it was written to make survives (the
    // heading leads the section, newest first, and nothing follows it); what
    // it drops is its brittleness about which lines PRECEDE it, which the
    // pairing check now sits in.
    let heading = lines
        .iter()
        .position(|line| {
            *line == format!("pns doctor: the last 2 decisions,{DECISION_HEADING_TAIL}")
        })
        .unwrap_or_else(|| panic!("no decision heading in {printed}"));
    assert!(
        lines[heading + 1].contains(" c2/done "),
        "the newest decision leads: {printed}"
    );
    assert!(lines[heading + 2].contains(" c1/done "), "{printed}");
    assert_eq!(
        lines[heading + 3],
        NONE_WAITING,
        "and only the journal's count follows it: {printed}"
    );
    assert_eq!(
        lines.len(),
        heading + 4,
        "with nothing after that: {printed}"
    );
}

#[test]
fn the_doctors_exit_code_does_not_move_for_a_log_that_is_absent_or_unreadable() {
    // THE SECTION REPORTS HISTORY, NOT HEALTH. An empty log on a fresh machine
    // is not a failure, and neither is one nothing can parse.
    let sandbox = Sandbox::new("doctor-decision-empty");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    assert!(
        printed.ends_with(&format!("{NO_DECISION_RECORDED}\n{NONE_WAITING}\n")),
        "{printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends earned a zero: {}",
        stderr(&output)
    );

    // The malformed legacy arm starts before its one-time import.
    let sandbox = Sandbox::new("doctor-decision-malformed");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    // A LINE NOBODY CAN PARSE is quoted back and still costs nothing.
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(sandbox.path("state/decisions"), "not a decision at all\n").expect("the ring");
    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    assert!(
        printed.ends_with(&format!(
            "  unreadable entry: \"not a decision at all\"\n{NONE_WAITING}\n"
        )),
        "{printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a malformed log is not a failed send: {}",
        stderr(&output)
    );
}

#[test]
fn a_ring_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code() {
    // ABSENT IS ITS OWN STATE, with its own honest line. This is the OTHER
    // one: something is at the path and the read failed, which is a different
    // thing to say and the only arm an absent file never exercises. A
    // directory is the portable way to produce a real read error.
    let sandbox = Sandbox::new("doctor-decision-unreadable");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    std::fs::create_dir_all(sandbox.path("state/decisions")).expect("a directory at the ring");

    let output = doctor_command(&sandbox).output().expect("the engine runs");
    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines.get(lines.len() - 2),
        Some(&NONE_WAITING),
        "the journal's count still comes after it: {printed}"
    );
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("pns doctor: state import decisions:")
    );
    let last = lines[lines.len() - 3];
    let opening = "pns doctor: the decision log could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NO_DECISION_RECORDED),
        "and it is never told as an absent log: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
}

#[test]
fn a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind() {
    // MEASURED: opening a FIFO BLOCKS until the other end is opened, for
    // READING as much as for writing, so a doctor that read the path raw
    // parks forever on a command a human is standing there waiting for. The
    // append side already refuses this path; the reader is the other half of
    // the same guard.
    let sandbox = Sandbox::new("doctor-decision-fifo");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    let ring = ring_path(&sandbox);
    plant_fifo(&ring);

    let mut command = doctor_command(&sandbox);
    let output = output_before_the_deadline(&mut command);

    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines.get(lines.len() - 2),
        Some(&NONE_WAITING),
        "the journal's count still comes after it: {printed}"
    );
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("pns doctor: state import decisions:")
    );
    let last = lines[lines.len() - 3];
    let opening = "pns doctor: the decision log could not be read (";
    assert!(last.starts_with(opening), "{printed}");
    assert!(
        last.ends_with(").") && last.len() > opening.len() + 2,
        "the kind is NAMED rather than left an empty parenthesis: {printed}"
    );
    assert!(
        !printed.contains(NO_DECISION_RECORDED),
        "and it is never told as an absent log: {printed}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the sends alone own the exit code: {}",
        stderr(&output)
    );
    // REFUSED, NOT REPAIRED, the way the append refuses it.
    assert!(
        std::fs::symlink_metadata(&ring)
            .expect("the fifo")
            .file_type()
            .is_fifo(),
        "the ring's path was rewritten"
    );
}

#[test]
fn the_doctor_records_no_decision_of_its_own() {
    // A DOCTOR THAT RECORDED would push the decision the operator came to read
    // out of the ring by the act of going to look at it.
    let sandbox = Sandbox::new("doctor-decision-readonly");
    sandbox.write_config(EVERY_DISPATCHED_CHANNEL);
    run(logged_event(&sandbox).args(["--agent", "claude", "--state", "done"]));
    let before = stored_records::text(&sandbox, "decisions");
    doctor_command(&sandbox).output().expect("the engine runs");
    assert_eq!(
        stored_records::text(&sandbox, "decisions"),
        before,
        "the doctor wrote to the ring it was reading"
    );
}
