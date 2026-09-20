use super::*;

#[test]
fn the_discovery_chain_is_one_spawn_and_one_walk() {
    // ONE SPAWN LEFT: `mosh-server` selection stays shelled because macOS
    // `pgrep -x` matches a name field the process table does not reproduce
    // exactly. The children and their terminals come from the walk, which
    // is handed the server ids it found.
    let table = FakeTable::naming(&[DISCOVERY_TERMINAL]);
    let asked = Arc::clone(&table.asked);
    let probes = phone_probe(&DISCOVERY, Arc::new(table));
    probes.phone_input_atime_secs();
    assert_eq!(
        probes.runner.calls.lock().unwrap().as_slice(),
        &["/usr/bin/pgrep -x mosh-server".to_string()]
    );
    assert_eq!(
        asked.lock().unwrap().as_slice(),
        &[vec![DISCOVERY_SERVER]],
        "the walk is asked once, about the servers pgrep named"
    );
}

#[test]
fn every_server_is_asked_about_in_one_walk() {
    // Two phones attached at once is the case that grows the id list, and
    // the walk takes the whole list, so the work stays one pass over the
    // process table however many sessions are open.
    let table = FakeTable::naming(&["ttys000", "ttys001"]);
    let asked = Arc::clone(&table.asked);
    let probes = phone_probe(
        &[("/usr/bin/pgrep -x mosh-server", "14362\n900\n")],
        Arc::new(table),
    );
    probes.phone_input_atime_secs();
    assert_eq!(asked.lock().unwrap().as_slice(), &[vec![14362, 900]]);
}

#[test]
fn a_failure_at_any_step_of_the_chain_reads_as_no_phone_rather_than_a_fresh_one() {
    // Never fresh is the fail direction: a phone that cannot be read must
    // drop out of the arbitration, not park the operator on it and silence
    // every banner.
    let unanswered = phone_probe(&[], Arc::new(FakeTable::naming(&[DISCOVERY_TERMINAL])));
    assert_eq!(
        unanswered.phone_input_atime_secs(),
        None,
        "the one spawn failing is no reading"
    );
    let no_children = phone_probe(&DISCOVERY, Arc::new(FakeTable::naming(&[])));
    assert_eq!(
        no_children.phone_input_atime_secs(),
        None,
        "a walk naming no terminal is no reading"
    );
}

#[test]
fn no_mosh_server_at_all_never_walks_for_children_of_nothing() {
    let table = FakeTable::naming(&[DISCOVERY_TERMINAL]);
    let asked = Arc::clone(&table.asked);
    let probes = phone_probe(&[("/usr/bin/pgrep -x mosh-server", "\n")], Arc::new(table));
    assert_eq!(probes.phone_input_atime_secs(), None);
    assert!(
        asked.lock().unwrap().is_empty(),
        "no server is nothing to walk from"
    );
}

#[test]
fn a_server_whose_client_has_no_terminal_reads_as_no_phone() {
    // A process with no controlling terminal carries no device number, so
    // the walk names nothing for it, and there is no clock on a terminal
    // that is not there.
    let probes = phone_probe(&DISCOVERY, Arc::new(FakeTable::naming(&[])));
    assert_eq!(probes.phone_input_atime_secs(), None);
}

#[test]
fn a_walk_that_never_returns_leaves_the_phone_reading_unknown() {
    // THE STALLED NATIVE CALL, phone side: the deadline is named so the
    // proof costs milliseconds rather than the production window.
    let table: Arc<dyn ProcessTable> = Arc::new(StalledTable);
    assert_eq!(
        phone_reading_within(
            &FakeRunner::answering("14362\n"),
            &table,
            "/dev",
            Duration::from_millis(20)
        ),
        None
    );
}
