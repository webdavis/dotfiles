use super::*;

#[test]
fn the_discovery_argv_is_pinned_to_the_chain_that_was_measured_live() {
    // THREE CALLS, in this order, with the ids batched rather than one
    // spawn per process. `mosh-server` itself has no controlling
    // terminal (measured: `??`), which is why the client is walked to at
    // all; a reordering or a dropped step ships a probe that silently
    // never reads a phone.
    let probes = phone_probe(&DISCOVERY);
    probes.phone_input_atime_secs();
    assert_eq!(
        probes.runner.calls.lock().unwrap().as_slice(),
        &[
            "/usr/bin/pgrep -x mosh-server".to_string(),
            "/usr/bin/pgrep -P 14362".to_string(),
            "/bin/ps -o tty= -p 14363".to_string(),
        ]
    );
}

#[test]
fn every_server_and_every_client_is_asked_for_in_one_call_each() {
    // Two phones attached at once is the case that grows the id lists,
    // and both pgrep and ps take the whole list, so the spawn count
    // stays at three however many sessions are open.
    let probes = phone_probe(&[
        ("/usr/bin/pgrep -x mosh-server", "14362\n900\n"),
        ("/usr/bin/pgrep -P 14362,900", "14363\n901\n"),
        ("/bin/ps -o tty= -p 14363,901", "ttys000 \nttys001 \n"),
    ]);
    probes.phone_input_atime_secs();
    assert_eq!(probes.runner.calls.lock().unwrap().len(), 3);
}

#[test]
fn a_failure_at_any_step_of_the_chain_reads_as_no_phone_rather_than_a_fresh_one() {
    // Never fresh is the fail direction: a phone that cannot be read
    // must drop out of the arbitration, not park the operator on it and
    // silence every banner. Each case drops one scripted answer, which
    // is that command failing.
    for dropped in [
        "/usr/bin/pgrep -x mosh-server",
        "/usr/bin/pgrep -P 14362",
        "/bin/ps -o tty= -p 14363",
    ] {
        let scripted: Vec<(&str, &str)> = DISCOVERY
            .iter()
            .copied()
            .filter(|(call, _)| *call != dropped)
            .collect();
        assert_eq!(
            phone_probe(&scripted).phone_input_atime_secs(),
            None,
            "case: {dropped} unanswered"
        );
    }
}

#[test]
fn no_mosh_server_at_all_never_asks_for_children_of_nothing() {
    // `pgrep -P` with an empty list is a usage error, not a query
    // answering "none", so the walk stops rather than spawning it.
    let probes = phone_probe(&[("/usr/bin/pgrep -x mosh-server", "\n")]);
    assert_eq!(probes.phone_input_atime_secs(), None);
    assert_eq!(
        probes.runner.calls.lock().unwrap().as_slice(),
        &["/usr/bin/pgrep -x mosh-server".to_string()]
    );
}

#[test]
fn a_server_whose_client_has_no_terminal_reads_as_no_phone() {
    // `ps -o tty=` prints `??` for a process with no controlling
    // terminal, and there is no clock on a terminal that is not there.
    let probes = phone_probe(&[
        ("/usr/bin/pgrep -x mosh-server", "14362\n"),
        ("/usr/bin/pgrep -P 14362", "14363\n"),
        ("/bin/ps -o tty= -p 14363", "??       \n"),
    ]);
    assert_eq!(probes.phone_input_atime_secs(), None);
}
