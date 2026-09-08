use super::*;

/// A LOOP THAT TRACED ITS TICK PASSES EVERY OTHER TEST HERE and fills the
/// operator's disk between two rotations: 86,400 lines a day rotates a real
/// log out of existence, and `compress-and-truncate-local-logs.sh` picks this
/// file up with no registration at all.
#[test]
fn the_daemon_does_not_write_a_log_line_per_tick() {
    let sandbox = Sandbox::new("daemon-does-not-chatter");
    pns_application::DecisionRing::read(&pns_adapters::SqliteStore::new(sandbox.state()))
        .expect("a healthy delivery store before daemon startup");
    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    // FIRST, THE EVIDENCE A TICK HAPPENED AT ALL: "said nothing" is vacuous
    // about a daemon that never got going, so wait for its own heartbeat
    // (written every tick, main.rs) before sleeping through more of them.
    assert!(
        poll_until(|| sandbox
            .state()
            .join("daemon-heartbeat")
            .exists()
            .then_some(()))
        .is_some(),
        "the daemon never beat; it said: {}",
        guard.said()
    );
    // THEN many more ticks, an empty spool, nothing to say.
    std::thread::sleep(Duration::from_millis(TICK_MS * 8));
    assert_eq!(guard.said(), "", "an idle daemon must say nothing at all");
}

/// THE SAME RULE ONE STEP FURTHER IN: a daemon that is doing its job says
/// nothing about having done it.
///
/// THE IDLE CASE ABOVE IS THE EASY HALF. The lights tick repeats every twelve
/// seconds for as long as its lease holds, so one line per firing is 300 lines
/// an hour of "ran `lights`" in the file
/// `compress-and-truncate-local-logs.sh` rotates a real log out of. A firing
/// that WORKED is not news; a spawn that failed still speaks, because an action
/// that suppressed its own error has not been performed.
#[test]
fn a_daemon_that_ran_a_job_says_nothing_about_having_run_it() {
    let sandbox = Sandbox::new("daemon-quiet-on-success");
    pns_application::DecisionRing::read(&pns_adapters::SqliteStore::new(sandbox.state()))
        .expect("a healthy delivery store before daemon startup");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let scheduled = schedule(&sandbox, &["--id", "drill", "--in", "0"], &EVENT);
    assert!(scheduled.status.success(), "{scheduled:?}");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| (fires(&sandbox) > 0).then_some(())).is_some(),
        "the job never fired; the daemon said: {}",
        guard.said()
    );
    // PAST THE SPAWN AND PAST THE DRAIN THAT FOLLOWS IT, so a line written
    // after the delivery landed is still inside the window this reads.
    std::thread::sleep(Duration::from_millis(TICK_MS * 8));
    assert_eq!(
        guard.said(),
        "",
        "a firing that worked is not news, and this job runs three to five \
         times a minute forever"
    );
}

/// THE ONLY READER A JOB CHILD HAS.
///
/// A job runs unattended with no terminal behind it, so a complaint it writes
/// on stderr goes wherever the daemon put that stream. With all three streams
/// null it went to `/dev/null`, and the lights tick's say-once memory then
/// recorded the complaint as SAID, so no later tick repeated it either: a lamp
/// renamed on the bridge was reported exactly once, into nothing. The plist
/// points both of the daemon's own streams at one log file, so inheriting is
/// what puts a child's line in front of the operator.
#[test]
fn a_job_childs_own_complaint_reaches_the_daemons_log() {
    let sandbox = Sandbox::new("daemon-child-stderr");
    sandbox.write_config(ONE_CHANNEL);
    // A BARE `pns lights` IS A USAGE ERROR: the shortest argv this binary
    // answers on stderr, said by the child and by nothing else in this test.
    let scheduled = schedule(&sandbox, &["--id", "noisy", "--in", "0"], &["lights"]);
    assert!(scheduled.status.success(), "{scheduled:?}");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| guard
            .said()
            .contains("usage: pns lights tick")
            .then_some(()))
        .is_some(),
        "the child's complaint never reached the log; the daemon said: {}",
        guard.said()
    );
}
