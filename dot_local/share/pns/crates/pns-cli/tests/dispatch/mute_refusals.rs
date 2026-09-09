use super::*;

#[test]
fn a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event() {
    // A READ ERROR IS NOT AN ABSENT FILE, and one `.ok()?` read both the same
    // way: an unreadable quiet-until muted nothing and said nothing, so the
    // operator had no way to learn the state file was broken. Fail open is
    // untouched; what changes is that it is announced.
    //
    // A DIRECTORY IN THE FILE'S PLACE is the portable vehicle. A chmod-000
    // file is not: a runner with enough privilege reads it anyway, and the
    // pin becomes a flake that depends on who ran the suite.
    let sandbox = Sandbox::new("quiet-unreadable");
    std::fs::create_dir_all(quiet_state(&sandbox)).expect("a directory where the file goes");

    let mut event = sandbox.pns();
    event.env("PNS_STATE_DIR", sandbox.path("state"));
    // At the desk with the pane out of sight and the card forced, the same
    // both-decorations row the corrupt-file pin uses, so a mute reading true
    // here would be unmissable.
    event.env("PNS_IDLE_SECS", "0");
    event.env("PNS_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut event, false);
    let output = run(event
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));

    assert!(
        sandbox.fired("macos-banner"),
        "an unreadable mute mutes nothing"
    );
    assert!(sandbox.fired("mobile"), "including a forced card");
    assert!(sandbox.fired("hermes"));
    let complaints = stderr(&output)
        .lines()
        .filter(|line| line.starts_with("pns: state error"))
        .map(String::from)
        .collect::<Vec<_>>();
    assert_eq!(
        complaints.len(),
        1,
        "one complaint per event, not one per reader: {}",
        stderr(&output)
    );
    // THE ERROR IS NAMED but not quoted verbatim: the operating system owns
    // that text, and pinning it would fail on a kernel that reworded it.
    assert!(
        complaints[0].starts_with("pns: state error (quiet-until could not be read: ")
            && complaints[0].ends_with("); nothing is muted, clear it with pns quiet off"),
        "the shape the parse complaint already uses, with the error inside: {}",
        complaints[0]
    );
}

#[test]
fn a_mute_that_could_not_be_written_reports_the_mute_that_still_stands() {
    // The legacy file fixture found a failed write reporting "not quiet"
    // while a later report still saw the original sixty-minute mute.
    // A writer holding the database now refuses a second write while readers
    // can still observe the committed mute. The failed command must report
    // that standing expiry, never the shorter mute it could not publish.
    let sandbox = Sandbox::new("quiet-write-fails-over-a-mute");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let standing = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 3_600;
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .set_quiet_expiry(Some(standing))
        .expect("the standing mute");
    let writer = quiet_records::writer(&sandbox);
    let output = quiet_command(&sandbox)
        .arg("30m")
        .output()
        .expect("the engine runs");
    drop(writer);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output)
            .lines()
            .any(|line| line.starts_with("pns: state error (quiet-until could not be written: ")),
        "loud about the write it could not make: {}",
        stderr(&output)
    );
    assert_eq!(
        stdout(&output).trim_end(),
        "pns: quiet for another 60 minutes",
        "the mute that still stands, not the one this run failed to set"
    );
    assert_eq!(
        quiet_records::expiry(&sandbox),
        Some(standing),
        "and the failed run moved nothing"
    );
}

#[test]
fn a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind() {
    // NOTHING PINNED THE EXIT CODE: `return 1` mutated to `return 0` survived
    // the whole suite. A caller reading a zero here treats a mute that never
    // landed as one that did, which is the failure this subcommand exits
    // non-zero at all to prevent.
    let sandbox = Sandbox::new("quiet-write-fails");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    set_state_mode(&sandbox, 0o500);
    let output = quiet_command(&sandbox)
        .arg("30m")
        .output()
        .expect("the engine runs");
    set_state_mode(&sandbox, 0o755);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output)
            .lines()
            .any(|line| line.starts_with("pns: state error (quiet-until could not be written: ")),
        "loud, never silent: {}",
        stderr(&output)
    );
    assert!(
        !sandbox.path("state/pns.db").exists(),
        "and no half-set mute left on disk"
    );
}

#[test]
fn a_publish_whose_rename_fails_leaves_no_pending_file_behind() {
    // The legacy rename-failure contract becomes a transaction refusal:
    // failed publication leaves neither a quiet row nor a pending file.
    // A real competing writer prevents this command from acquiring ownership.
    let sandbox = Sandbox::new("quiet-rename-fails");
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .set_quiet_expiry(None)
        .expect("the initialized empty mute");
    let writer = quiet_records::writer(&sandbox);
    let output = quiet_command(&sandbox)
        .arg("30m")
        .output()
        .expect("the engine runs");

    drop(writer);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        quiet_records::expiry(&sandbox).is_none(),
        "no half-published quiet row"
    );
    let pending = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("quiet-until.new."))
        .collect::<Vec<_>>();
    assert!(
        pending.is_empty(),
        "left in the state directory: {pending:?}"
    );
}
