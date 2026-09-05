//! What one run amounts to: its exit code, what the record carries, and when the marker moves.

mod support;

use support::*;

/// The epoch at the head of a marker or a stub's breadcrumb.
fn epoch_in(text: &str) -> i64 {
    text.split_whitespace()
        .next()
        .expect("an epoch field")
        .parse()
        .expect("an epoch")
}

#[test]
fn a_machine_with_no_config_updates_nothing_and_exits_clean() {
    let home = Home::new("no-config");
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("no config"), "{output:?}");
    assert!(
        !home.marker().exists(),
        "a run that did nothing must not invent a success"
    );
}

#[test]
fn a_lane_asked_for_by_name_on_a_configless_machine_is_refused_with_exit_one() {
    // The bare run above is clean by design; a lane named on the command line
    // is a request, and a request that could not run is not a success.
    let home = Home::new("no-config-by-name");
    let output = home.uu(&["run", "herdr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("no config"), "{output:?}");
    assert!(!home.marker().exists(), "{output:?}");
}

#[test]
fn a_lane_the_config_never_declares_is_refused_by_name_with_exit_one() {
    // With no static roster of names left, this is the ONLY guard on `uu run
    // <lane>`: a name nothing declares must exit non-zero and stamp no
    // success, or a typo reads as a run that worked.
    let home = Home::new("undeclared").with_herdr_lane(0);
    let output = home.uu(&["run", "hedr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("no `[lanes.hedr]` block"),
        "{output:?}"
    );
    assert!(!home.marker().exists(), "{output:?}");
}

#[test]
fn a_clean_run_exits_zero_and_advances_the_marker() {
    let home = Home::new("clean").with_herdr_lane(0);
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(home.marker().exists(), "{output:?}");
}

#[test]
fn a_failed_lane_leaves_the_exit_at_zero_and_the_marker_unmoved() {
    // CONTINUE ON FAILURE is the run's contract: the record is what reports
    // the failure, and the next entry's gap has to measure the last time
    // everything actually worked.
    let home = Home::new("failed").with_herdr_lane(1);
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("failure"), "{output:?}");
    assert!(!home.marker().exists(), "{output:?}");
}

#[test]
fn a_deferred_command_lane_leaves_the_marker_unmoved_even_with_zero_failures() {
    // THE MUTANT THAT MATTERS MOST (brief D7): a deferral must never look
    // like the clean run that advances the marker, or a lane that never
    // truly runs reads as healthy forever. `failures` alone is 0 here, so
    // this is the one case that catches a marker gate that forgot to also
    // check `deferred`.
    let home = Home::new("deferred-marker");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\n\
         printf 'nothing was attempted\\n'\n\
         printf 'another run holds the lock\\n' >&2\n\
         exit 75\n",
    );
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let record = stdout(&output);
    assert!(record.contains("mine: deferred"), "{record}");
    assert!(
        !record.contains("mine: 0 failure(s)"),
        "a deferral must not read as a clean lane: {record}"
    );
    // The child's own STDOUT and STDERR text, not just the word "deferred":
    // a mutant that blanks stdout on this path, or replaces the deferred
    // reason with a fixed string, would still satisfy every assertion above.
    assert!(record.contains("nothing was attempted"), "{record}");
    assert!(record.contains("another run holds the lock"), "{record}");
    assert!(
        !home.marker().exists(),
        "a deferred lane did no work and must not advance the marker: {output:?}"
    );
}

#[test]
fn a_deferral_leaves_the_marker_the_last_success_wrote_exactly_as_it_was() {
    // D7's OTHER half. The test above pins a marker that was never written;
    // this pins one that WAS. A mutant that CLEARS the marker on a deferral
    // satisfies "the marker did not advance" and still destroys the window
    // the next entry measures its gap from, so the machine reports "last
    // successful run: NEVER RECORDED" a week after a run that succeeded.
    let home = Home::new("deferred-keeps-marker");
    let counter = home.dir.join("call-count");
    let stub = home.write_stub(
        "updater",
        &format!(
            "cat >/dev/null\n\
             count=$(( $(cat {counter:?} 2>/dev/null || printf 0) + 1 ))\n\
             printf '%s' \"$count\" >{counter:?}\n\
             if [ \"$count\" -eq 1 ]; then exit 0; fi\n\
             printf 'lock held\\n' >&2\n\
             exit 75\n",
        ),
    );
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    home.uu(&["run"]);
    let after_success = std::fs::read_to_string(home.marker()).expect("the first run succeeded");
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("mine: deferred"), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(home.marker()).ok().as_deref(),
        Some(after_success.as_str()),
        "a deferral must leave the last success where it was, neither advanced nor erased"
    );
}

#[test]
fn a_deferred_command_lane_never_fires_the_per_run_failure_alert() {
    // D2: deferral is not a failure, so it must never reach the same alert
    // path a failed lane's non-zero exit does.
    let home = Home::new("deferred-no-alert");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        !home.dir.join("alert-args").exists(),
        "a deferred lane must not fire the per-run failure alert"
    );
}

#[test]
fn a_mixed_run_records_each_lanes_own_verdict_alerts_only_the_failed_one_and_stays_exit_zero() {
    let home = Home::new("mixed-run");
    let clean = home.write_stub(
        "clean-updater",
        "cat >/dev/null\nprintf 'clean lane ok\\n'\n",
    );
    let failing = home.write_stub(
        "failing-updater",
        "cat >/dev/null\nprintf 'boom: disk full\\n' >&2\nexit 2\n",
    );
    let deferring = home.write_stub(
        "deferring-updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n'\nprintf 'lock held\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.a-clean]\ntype = \"command\"\nrun = [\"{}\"]\n\n\
         [lanes.b-failing]\ntype = \"command\"\nrun = [\"{}\"]\n\n\
         [lanes.c-deferring]\ntype = \"command\"\nrun = [\"{}\"]\n\n\
         [alerts]\nbinary = \"{}\"\n",
        clean.display(),
        failing.display(),
        deferring.display(),
        pns_stub.display(),
    ));
    let output = home.uu(&["run"]);
    // No consumer reads uu's own exit status today (D5), so a mixed run
    // stays exit 0 exactly like an all-failed run: nothing is invented for a
    // code nothing checks.
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let record = stdout(&output);
    assert!(record.contains("a-clean: 0 failure(s)"), "{record}");
    assert!(record.contains("b-failing: 1 failure(s)"), "{record}");
    assert!(record.contains("c-deferring: deferred"), "{record}");
    assert!(
        record.contains("=== done, 1 failure(s), 1 deferred ==="),
        "{record}"
    );
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert!(alerts.contains("b-failing"), "{alerts}");
    assert!(
        !alerts.contains("c-deferring"),
        "the deferred lane must not alert: {alerts}"
    );
    assert!(
        !home.marker().exists(),
        "a run carrying any failure or deferral must not advance the marker: {output:?}"
    );
}

#[test]
fn a_lane_that_outlives_its_deadline_fails_instead_of_holding_the_run_open() {
    // THE WIRE, which no unit test reaches: the deadline the LANE'S OWN BLOCK
    // declares has to be the one the spawn is bounded by. A run handed some
    // other duration would wait the stub out and report a clean self-update.
    //
    // The stub leaves a `sleep` behind holding its stdout and exits at once,
    // which is the hang this bounds: waiting on the child answers immediately
    // and the READ is what blocks. Its 4 seconds outlast the 1-second deadline
    // by enough that a run finishing here finished because uu stopped it.
    let home = Home::new("lane-deadline").with_herdr_lane_and(
        "sleep 4 &
exit 0
",
        "deadline_secs = 1
",
    );
    // STRUCTURAL: `deadline_secs` is whole seconds, so one second is the
    // shortest deadline a config can state and the shortest this can take.
    home.allow_slow("the smallest deadline a config can express is one second");
    let output = home.uu(&["run"]);
    let said = stdout(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a lane failure is not a run failure: {output:?}"
    );
    assert!(
        said.contains("lane `herdr` exceeded its 1s deadline"),
        "{said}"
    );
    assert!(said.contains("1 failure(s)"), "{said}");
}

#[test]
fn a_record_the_gateway_never_received_leaves_the_marker_unmoved() {
    // The marker is what the NEXT record measures its gap from, so a run
    // stamped successful after its entry was refused makes the following entry
    // claim a gap from a week nothing can read. The lanes themselves pass
    // here: the record path alone decides.
    let home = Home::new("record-refused").with_herdr_lane_and(
        "exit 0\n",
        &format!(
            "\n[records]\nurl = \"http://127.0.0.1:{}/uu\"\nkey = \"k\"\n",
            closed_port()
        ),
    );
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("post FAILED"),
        "the refused delivery is said out loud: {}",
        stdout(&output)
    );
    assert!(
        !home.marker().exists(),
        "a run whose record never landed must not stamp a success: {output:?}"
    );
}

#[test]
fn a_deferred_only_run_posts_a_record_body_stated_deferred_not_completed() {
    // FINDING 1 (6v): the earlier test for this call site
    // (`a_deferred_only_run_posts_a_body_stated_deferred_not_completed` in
    // `record.rs`) calls `records_body(0, 1, "detail")` directly, so it can
    // never see sol's mutant at the CALL SITE in `run_mode`
    // (`records_body(failures, 0, &detail)`), which passes all 189 tests.
    // This spawns the real binary and inspects the body its production
    // `UreqSignedPost` call actually puts on the wire, the same way the
    // review itself proved the mutant with a loopback listener.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("its number");
    let server = std::thread::spawn(move || read_posted_body(listener));

    let home = Home::new("records-deferred-state");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[records]\nurl = \"http://{addr}/uu\"\nkey = \"k\"\n",
        stub.display(),
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let body = server.join().expect("the listener thread");
    assert!(
        body.contains("\"state\":\"deferred\""),
        "a run with only a deferred lane and zero failures must post state \
         `deferred`, never `completed`: {body}"
    );
}

#[test]
fn the_marker_stamps_when_the_run_finished_and_not_when_it_started() {
    // Every record's gap is measured from this timestamp, so a marker holding
    // the run's START time inflates the next gap by the whole duration of the
    // run before it, and lanes have no upper bound. Crossing a wall-clock
    // second inside the lane is the only observation that tells the two
    // instants apart, so the stub spends its update call doing exactly that
    // and leaves behind the second it began in.
    let home = Home::new("finish-time").with_herdr_lane_and(
        "case \"$1\" in\n\
         update)\n\
         date +%s >\"$HOME/lane-started\"\n\
         began=$(date +%s)\n\
         while [ \"$(date +%s)\" = \"$began\" ]; do sleep 0.05; done\n\
         ;;\n\
         esac\n\
         exit 0\n",
        "",
    );
    // STRUCTURAL: the marker's own resolution is whole seconds, so telling
    // the run's start from its finish needs a real second boundary between
    // them; the spin above is 0 to just-under-1s by construction, not a
    // number this test controls.
    home.allow_slow(
        "the marker's resolution is whole seconds; crossing one is the only reliable signal",
    );
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let began =
        epoch_in(&std::fs::read_to_string(home.dir.join("lane-started")).expect("the lane"));
    let stamped = epoch_in(&std::fs::read_to_string(home.marker()).expect("the marker"));
    assert!(
        stamped > began,
        "the marker stamped {stamped}, which is not after the second the lane began in ({began})"
    );
}
