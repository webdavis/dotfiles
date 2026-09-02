//! The daemon driven as a process: the loop, the spool drain, the spawn and
//! the reap.
//!
//! EVERY TEST HERE RUNS THE REAL BINARY through `DaemonGuard`, which kills it
//! on every exit path including a panic, and every one of them pins the state
//! directory inside its own sandbox. The pure decisions (what a job is,
//! whether it fires, what a repeat re-arms to) are unit tested in
//! `src/daemon.rs`; nothing here re-proves one.

mod support;

use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use support::{DaemonGuard, Sandbox, poll_until, run, stdout};

/// A tick fast enough that a whole test costs a fraction of a second.
const TICK_MS: u64 = 25;

/// The floor `main.rs`'s `MIN_TICK_MS` accepts: below this the daemon
/// silently falls back to its one-SECOND production default, which would
/// make a test slower rather than faster. Used only by the two tests whose
/// cost is `SWITCH_TICKS` or `CHILD_TICKS` (both 30) ticks deep, where
/// `TICK_MS` costs 750 ms; at this floor the same wait is 300 ms.
const FAST_TICK_MS: u64 = 10;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock")
        .as_secs()
}

/// The hermes stub replaced by one that APPENDS a line per delivery, so "once"
/// and "twice" are different observations. The shared stub truncates, which
/// makes a second firing indistinguishable from the first.
fn count_fires(sandbox: &Sandbox) {
    sandbox.stub_channel(
        "hermes",
        &format!("cat >>\"{}/hermes.events\"", sandbox.display()),
    );
}

/// How many times the counting stub has been handed an event.
fn fires(sandbox: &Sandbox) -> usize {
    std::fs::read_to_string(sandbox.path("hermes.events"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// One registration through the typed command, which is the same library call
/// a rider will make.
fn schedule(sandbox: &Sandbox, flags: &[&str], args: &[&str]) -> std::process::Output {
    let mut command = sandbox.pns_stateful();
    command.args(["daemon", "schedule"]);
    command.args(flags);
    command.arg("--");
    command.args(args);
    command.output().expect("the engine runs")
}

/// The one channel a scheduled job's event reaches.
///
/// A CONFIG IS NOT OPTIONAL here: with none, the re-executed child selects no
/// plugin at all, so the daemon would report a job run and nothing would be
/// delivered. HERMES rather than the banner, because the sandbox pins the
/// operator AWAY (`PNS_IDLE_SECS` at a day), and a banner on a screen nobody is
/// sitting at is exactly what the engine declines to raise.
const ONE_CHANNEL: &str = "[plugins.hermes]\nenabled = true\nkey = \"k\"\n";

/// An ordinary event for a scheduled job to deliver.
const EVENT: [&str; 6] = [
    "--agent",
    "pns",
    "--state",
    "done",
    "--detail",
    "a scheduled job",
];

/// Whatever is left in the spool.
fn spooled(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sandbox.state().join("daemon"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// THE TEST THAT PROVES THE FEATURE EXISTS END TO END: something was scheduled,
/// nothing else happened, and a card came out of it.
#[test]
fn a_scheduled_job_runs_once_and_its_effect_is_observable() {
    let sandbox = Sandbox::new("daemon-runs-a-job");
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
    // The spool is drained by the run, and a one-shot leaves nothing behind.
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the spool still holds {:?}",
        spooled(&sandbox)
    );
    // ONCE, and it stays once: a one-shot that re-armed would keep firing.
    std::thread::sleep(Duration::from_millis(TICK_MS * 8));
    assert_eq!(fires(&sandbox), 1, "a one-shot fires exactly once");
}

/// THE ANIMATION-UPKEEP PRIMITIVE, proven as a behavior rather than as a
/// function: it repeats, and then the lease stops it without anybody saying so.
#[test]
fn a_repeating_job_keeps_firing_until_its_lease_runs_out_then_stops() {
    let sandbox = Sandbox::new("daemon-repeats-until-the-lease");
    // STRUCTURAL, at ~5.4 s: the `--until +3` lease below (the comment there
    // explains why +1 flaked), plus `every` at MIN_EVERY_SECS (1 s,
    // daemon.rs: an epoch-second lease cannot lapse faster), plus a 1.2 s
    // settle that has to outlast one `every` to prove firing really stopped.
    // Three numbers add to the floor; the settle alone understates it by
    // four seconds.
    sandbox.allow_slow(
        "a 3s lease, a 1s minimum `every`, and a 1.2s settle past one `every`: ~5.4s, not just the settle",
    );
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let scheduled = schedule(
        &sandbox,
        // `--until +3` RATHER THAN `+1`. At `+1` the whole margin was "the
        // daemon starts inside the same whole second the schedule read": a
        // first drain landing in the next second fires once, re-arms past its
        // own lease, and `fires >= 2` can then never be satisfied, so the test
        // burns its ten-second deadline and fails. Measured 1 in 10 red with
        // 200ms of injected start-up delay and 5 in 10 at 300ms, on a machine
        // faster than the shared CI runner. Three seconds still proves both
        // halves: it repeats, and the lease is what stops it.
        &[
            "--id", "upkeep", "--in", "0", "--every", "1", "--until", "+3",
        ],
        &EVENT,
    );
    assert!(scheduled.status.success(), "{scheduled:?}");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| (fires(&sandbox) >= 2).then_some(())).is_some(),
        "a repeat fired {} times; the daemon said: {}",
        fires(&sandbox),
        guard.said()
    );
    // The lease runs out and the daemon drops the job of its own accord.
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the lease never expired the job: {:?}",
        spooled(&sandbox)
    );
    // THE LAST OCCURRENCE IS STILL BEING DELIVERED when the spool empties: the
    // daemon re-arms and spawns, and it never waits for the child, so the
    // firing that emptied the spool has not been recorded yet. The daemon says
    // NOTHING about a firing that worked, so the count settles itself: a window
    // longer than one `every` with no change across it is a delivery that has
    // landed and a lease that is really gone. The window is what a repeat
    // outliving its own lease would show up in.
    let settled = poll_until(|| {
        let before = fires(&sandbox);
        std::thread::sleep(Duration::from_millis(1_200));
        (fires(&sandbox) == before).then_some(before)
    })
    .expect("the firing count never stopped rising");
    assert!(
        settled >= 2,
        "the repeat fired {settled} times before the lease stopped it"
    );
}

/// THE POINT IS THE ABSENCE OF A WAIT. A registration that talked to the daemon
/// would hold its caller for as long as the daemon was wedged, which is the
/// class the whole design exists to stay out of.
#[test]
fn a_registration_succeeds_with_no_daemon_anywhere_and_blocks_on_nothing() {
    let sandbox = Sandbox::new("daemon-registration-never-waits");
    let started = Instant::now();
    let scheduled = schedule(&sandbox, &["--id", "lonely", "--in", "600"], &EVENT);
    let elapsed = started.elapsed();
    assert!(scheduled.status.success(), "{scheduled:?}");
    assert_eq!(spooled(&sandbox), vec!["lonely".to_string()]);
    // A GENEROUS CEILING, never a tight number: this asserts that nothing
    // waits, not how fast a process starts on a loaded machine.
    assert!(
        elapsed < Duration::from_secs(5),
        "the registration took {elapsed:?}, which is a wait on something"
    );
}

/// WHERE A NAIVE `wait()` WOULD PASS EVERY OTHER TEST AND HANG IN PRODUCTION.
#[test]
fn a_hung_child_does_not_stall_the_tick_and_is_killed() {
    let sandbox = Sandbox::new("daemon-hung-child");
    // ONE STUB, TWO BEHAVIORS, told apart by the event it was handed: the
    // hanging job records the pns process that ran it and then hangs far past
    // the daemon's bound, and any other job records that it arrived.
    sandbox.write_config(ONE_CHANNEL);
    // The hanging job also starts a GRANDCHILD of the daemon's own child and
    // records its pid, because that is what a real delivery is: the job is a
    // `pns` that spawns a channel and waits on it. Killing only the direct
    // child leaves the delivery running.
    sandbox.stub_channel(
        "hermes",
        &format!(
            "event=$(cat)\n\
             if [[ $event == *hangs* ]]; then\n\
             sleep 30 &\n\
             printf '%s' \"$!\" >\"{sandbox}/hung.grandchild\"\n\
             printf '%s' \"$PPID\" >\"{sandbox}/hung.ppid\"\n\
             wait\n\
             else\n\
             printf '%s' \"$event\" >\"{sandbox}/second.event\"\n\
             fi",
            sandbox = sandbox.display()
        ),
    );

    assert!(
        schedule(
            &sandbox,
            &["--id", "hangs", "--in", "0"],
            &["--agent", "pns", "--state", "done", "--detail", "hangs"],
        )
        .status
        .success()
    );
    // FAST_TICK_MS, not TICK_MS: the kill bound below is CHILD_TICKS (30)
    // ticks deep, so the ordinary tick would cost 750 ms proving the same
    // thing this floor proves in 300.
    let guard = DaemonGuard::start(&sandbox, FAST_TICK_MS);
    let hung = poll_until(|| std::fs::read_to_string(sandbox.path("hung.ppid")).ok())
        .expect("the hung job never started");

    // THE TICK IS NOT STALLED: a second job registered while the first is
    // still hanging still fires.
    assert!(
        schedule(&sandbox, &["--id", "ordinary", "--in", "0"], &EVENT)
            .status
            .success()
    );
    assert!(
        poll_until(|| sandbox.path("second.event").exists().then_some(())).is_some(),
        "the second job never fired; the daemon said: {}",
        guard.said()
    );

    // AND THE WHOLE GROUP IS KILLED PAST ITS BOUND rather than left to
    // accumulate. The direct child first, which any kill would reach.
    assert!(
        poll_until(|| (!process_lives(&hung)).then_some(())).is_some(),
        "the pns process the hung job started ({hung}) outlived the daemon's bound"
    );
    // AND THEN THE DELIVERY IT WAS WAITING ON, which is the half a kill aimed
    // at the direct child alone leaves running: measured still alive 750ms past
    // a 300ms bound, and a repeating hung job accumulates one every occurrence.
    let grandchild = std::fs::read_to_string(sandbox.path("hung.grandchild"))
        .expect("the hung job never recorded its own child");
    assert!(
        poll_until(|| (!process_lives(&grandchild)).then_some(())).is_some(),
        "the delivery the hung job started ({grandchild}) outlived the daemon's bound"
    );
}

/// Whether a pid is still around, asked without a signal of our own: `kill -0`
/// sends nothing and only reports existence.
fn process_lives(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", pid])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// A LOOP THAT TRACED ITS TICK PASSES EVERY OTHER TEST HERE and fills the
/// operator's disk between two rotations: 86,400 lines a day rotates a real
/// log out of existence, and `compress-and-truncate-local-logs.sh` picks this
/// file up with no registration at all.
#[test]
fn the_daemon_does_not_write_a_log_line_per_tick() {
    let sandbox = Sandbox::new("daemon-does-not-chatter");
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

/// THE DOCTOR'S EXIT CODE DOES NOT MOVE, in the state where it would be most
/// tempting to move it.
///
/// ADDED BEYOND THE BRIEF'S FIFTEEN, which asks for exactly this if no
/// assertion already covers it: `exit_code` cannot see the daemon at all, so
/// the only place the mistake could be made is the composition root, and only
/// a run of the real binary reaches that.
#[test]
fn the_doctor_reports_a_dead_daemon_without_moving_its_exit_code() {
    let sandbox = Sandbox::new("daemon-doctor-line");
    sandbox.write_config("[plugins.macos-banner]\nenabled = true\n");
    let mut command = sandbox.pns_stateful();
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    let output = run(command.arg("doctor"));
    assert!(
        stdout(&output).contains("pns doctor: the daemon is enabled and has not run yet"),
        "the doctor must report the clock: {}",
        stdout(&output)
    );

    // And with a beat too old to vouch for, it still says so and still exits 0.
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    std::fs::write(
        state.join("daemon-heartbeat"),
        format!("4321 {}\n", now_secs() - 3_600),
    )
    .expect("a stale heartbeat");
    let mut command = sandbox.pns_stateful();
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    let output = run(command.arg("doctor"));
    assert!(
        stdout(&output).contains("so it is not running"),
        "a stale beat must read as not running: {}",
        stdout(&output)
    );
}

/// A GUARD, not a red-first behavior: it passes on the first commit and its
/// job is to keep passing.
///
/// THE SLICE'S ENTIRE SAFETY CLAIM. Every notification pns delivers today must
/// be delivered identically with a daemon running: the same stdout byte for
/// byte, the same stderr, the same exit code, AND THE SAME CHANNELS FIRED.
/// Kept over the usual objection to guard tests because its failure is silent
/// and lands in the harness, where a changed byte on a hook's stdout is a
/// permission prompt that stops being drawn.
///
/// THE FIRED SET IS PART OF THE CLAIM. A hook whose stdout and exit code were
/// untouched while a delivery quietly stopped going out has failed the
/// fail-open property exactly as badly, and only the channel legs can see it.
#[test]
fn the_daemon_changes_nothing_about_a_hook() {
    let sandbox = Sandbox::new("daemon-changes-no-hook");
    sandbox.write_config(ONE_CHANNEL);

    // The same two hooks, run first with no daemon anywhere.
    let quiet = [run_hook(&sandbox, "stop"), run_hook(&sandbox, "blocked")];

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    // Past a few ticks, so the daemon is unmistakably up and beating.
    std::thread::sleep(Duration::from_millis(TICK_MS * 4));
    let noisy = [run_hook(&sandbox, "stop"), run_hook(&sandbox, "blocked")];

    for (event, (before, after)) in ["stop", "blocked"]
        .iter()
        .zip(quiet.iter().zip(noisy.iter()))
    {
        assert_eq!(
            before.0.stdout,
            after.0.stdout,
            "{event}: a daemon changed the hook's stdout; it said: {}",
            guard.said()
        );
        assert_eq!(
            String::from_utf8_lossy(&before.0.stderr),
            String::from_utf8_lossy(&after.0.stderr),
            "{event}: a daemon changed the hook's stderr; it said: {}",
            guard.said()
        );
        assert_eq!(
            before.0.status.code(),
            after.0.status.code(),
            "{event}: a daemon changed the hook's exit code"
        );
        assert_eq!(
            before.1, after.1,
            "{event}: a daemon changed which channels the hook delivered to"
        );
    }
}

/// The channels this sandbox's stubs recorded a delivery on, cleared first so
/// the answer belongs to ONE hook run rather than to every run before it.
const RECORDING: [&str; 3] = ["mobile", "hermes", "macos-banner"];

fn run_hook(sandbox: &Sandbox, event: &str) -> (std::process::Output, Vec<&'static str>) {
    for channel in RECORDING {
        let _ = std::fs::remove_file(sandbox.path(&format!("{channel}.event")));
    }
    let payload = match event {
        "stop" => r#"{"session_id":"s1","last_assistant_message":"done here"}"#,
        _ => r#"{"session_id":"s1","message":"needs approval"}"#,
    };
    let output = hook(sandbox, event, payload);
    let fired = RECORDING
        .into_iter()
        .filter(|channel| sandbox.fired(channel))
        .collect();
    (output, fired)
}

/// A FIFO IN THE SPOOL MUST NOT STALL THE CLOCK. `open` on one blocks until a
/// writer arrives, so a daemon that opened what it found would stop forever on
/// the first tick that saw it, with nothing in the log to say why.
///
/// ADDED BEYOND THE BRIEF'S FIFTEEN: the brief specifies this as one of the
/// loop's four refusals rather than as a behavior, and a refusal whose failure
/// is an unkillable hang is worth the one test.
#[test]
fn an_irregular_spool_entry_is_left_alone_and_never_opened() {
    let sandbox = Sandbox::new("daemon-refuses-a-fifo");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let spool = sandbox.state().join("daemon");
    std::fs::create_dir_all(&spool).expect("the spool");
    let fifo = spool.join("not-a-job");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|status| status.success()),
        "the test needs a real FIFO"
    );

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        schedule(&sandbox, &["--id", "ordinary", "--in", "0"], &EVENT)
            .status
            .success()
    );
    assert!(
        poll_until(|| (fires(&sandbox) > 0).then_some(())).is_some(),
        "the FIFO stalled the clock; the daemon said: {}",
        guard.said()
    );
    // Left exactly where it was found, and complained about once rather than
    // once a tick.
    assert!(fifo.exists(), "an irregular entry is never removed");
    assert_eq!(
        guard
            .said()
            .lines()
            .filter(|line| line.contains("not a regular file"))
            .count(),
        1,
        "said once, not once a tick: {}",
        guard.said()
    );
}

/// M17'S BEHAVIOR, WHICH NOTHING EXERCISED: a marker file actually on disk
/// cancels a scheduled job.
///
/// The decision function's boolean was unit tested, and `marker_exists` and
/// `marker_dir` had no reference in any test at all, so pointing the markers
/// directory at a name that does not exist survived the whole suite. This is
/// the nag's entire cancellation primitive, and the nag slice is queued
/// directly on top of this one.
#[test]
fn a_marker_on_disk_cancels_a_scheduled_job_end_to_end() {
    let sandbox = Sandbox::new("daemon-marker-cancels");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let markers = sandbox.state().join("daemon-markers");
    std::fs::create_dir_all(&markers).expect("the markers directory");
    std::fs::write(markers.join("answered"), "").expect("the marker");

    assert!(
        schedule(
            &sandbox,
            &["--id", "nag", "--in", "0", "--unless-marker", "answered"],
            &EVENT,
        )
        .status
        .success()
    );
    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the cancelled job was never taken out of the spool: {:?}",
        spooled(&sandbox)
    );
    assert!(
        guard.said().contains("its marker was already there"),
        "the drop must name the marker as the reason: {}",
        guard.said()
    );

    // THE IN-TEST CONTROL: the same daemon, the same tick, an identical job
    // with no marker naming it. It fires, so the silence above is the marker
    // and not a daemon that was never running.
    assert!(
        schedule(&sandbox, &["--id", "ordinary", "--in", "0"], &EVENT)
            .status
            .success()
    );
    assert!(
        poll_until(|| (fires(&sandbox) == 1).then_some(())).is_some(),
        "the unmarked control never fired; the daemon said: {}",
        guard.said()
    );
    std::thread::sleep(Duration::from_millis(TICK_MS * 8));
    assert_eq!(
        fires(&sandbox),
        1,
        "only the unmarked job may have fired; the daemon said: {}",
        guard.said()
    );
}

/// M18'S BEHAVIOR: a hand-edited spool record that would not have passed
/// registration is dropped rather than run.
///
/// The loop re-applies the registration's own validation to what it reads back,
/// which is what stops a file written by hand from doing what a registration
/// could not. Nothing wrote a malformed spool file, so deleting that re-check
/// survived the whole suite.
#[test]
fn a_hand_edited_spool_record_whose_args_fail_validation_is_dropped() {
    let sandbox = Sandbox::new("daemon-drops-a-hand-edited-record");
    sandbox.write_config(ONE_CHANNEL);
    count_fires(&sandbox);
    let spool = sandbox.state().join("daemon");
    std::fs::create_dir_all(&spool).expect("the spool");
    let now = now_secs();
    // Parses cleanly and is refused by the shape rules: an empty argv is a job
    // that would re-execute pns with no event at all.
    std::fs::write(
        spool.join("handmade"),
        format!(
            "id=handmade	due={now}	until={}	args=[]
",
            now + 300
        ),
    )
    .expect("the hand-edited record");

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    assert!(
        poll_until(|| spooled(&sandbox).is_empty().then_some(())).is_some(),
        "the invalid record was left in the spool: {:?}",
        spooled(&sandbox)
    );
    assert!(
        guard.said().contains("dropped `handmade`") && guard.said().contains("`args` is empty"),
        "the drop must name the record and the rule it broke: {}",
        guard.said()
    );
    assert_eq!(fires(&sandbox), 0, "a refused record must never be run");
}

/// M20'S BEHAVIOR: `enabled = false` stops a daemon that is ALREADY RUNNING.
///
/// Read once at startup the switch was inert. Nothing bounces this launchd job
/// when the config changes (the loader's trigger is the plist hash), so the
/// operator's off switch did nothing at all until a hand-typed bootout, while
/// the daemon kept firing jobs and the doctor reported it off.
///
/// EXIT 0 IS HALF THE BEHAVIOR: `KeepAlive { SuccessfulExit = false }` is what
/// keeps a clean exit exited, and a non-zero one here would relaunch the job
/// every ten seconds forever.
#[test]
fn turning_the_config_switch_off_stops_a_running_daemon() {
    let sandbox = Sandbox::new("daemon-off-switch-is-real");
    sandbox.write_config(&format!(
        "{ONE_CHANNEL}[daemon]
enabled = true
"
    ));
    // FAST_TICK_MS, not TICK_MS: the switch is re-read every SWITCH_TICKS
    // (30) ticks, so the ordinary tick would cost 750 ms proving the same
    // thing this floor proves in 300.
    let mut guard = DaemonGuard::start(&sandbox, FAST_TICK_MS);
    // Up and beating before the switch moves, so the exit below is the config
    // and not a daemon that never started.
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

    sandbox.write_config(&format!(
        "{ONE_CHANNEL}[daemon]
enabled = false
"
    ));
    let status = guard
        .exited_within(Duration::from_secs(10))
        .expect("the daemon kept running after the switch was turned off");
    assert_eq!(
        status.code(),
        Some(0),
        "a switched-off daemon must exit cleanly so launchd keeps it down"
    );
    assert!(
        guard.said().contains("disabled in the config; exiting"),
        "the exit must say why: {}",
        guard.said()
    );
}

/// M21'S BEHAVIOR: a spool path that is not a directory refuses the start, and
/// the refusal EXITS 0.
///
/// Neither startup refusal can heal by retrying, and under
/// `KeepAlive { SuccessfulExit = false }` a non-zero exit is relaunched every
/// ten seconds forever: ~8,640 relaunches and ~8,640 copies of this line a day,
/// which is the chatter the no-log-per-tick behavior exists to prevent arriving
/// through the restart door instead.
#[test]
fn a_spool_that_is_not_a_directory_refuses_the_start_and_exits_zero() {
    let sandbox = Sandbox::new("daemon-refuses-a-spool-file");
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    std::fs::write(state.join("daemon"), "not a directory").expect("a file in the way");

    let output = sandbox
        .pns_stateful()
        .env("PNS_DAEMON_TICK_MS", TICK_MS.to_string())
        .args(["daemon", "run"])
        .output()
        .expect("the engine runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a refusal retrying cannot fix must not be relaunched every ten seconds"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not a directory; refusing to start"),
        "the refusal must say what it found: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A FIFO WHERE THE HEARTBEAT SHOULD BE MUST NOT HANG THE DOCTOR.
///
/// `open` on a named pipe blocks until a writer arrives, so a doctor that read
/// whatever it found there would never reach its own exit code, its pairing
/// check, or any of the lines below this one. Bounded here, so the regression
/// reads as a failed assertion rather than as a run that never ends.
#[test]
fn a_heartbeat_that_is_not_a_regular_file_is_refused_rather_than_opened() {
    let sandbox = Sandbox::new("daemon-doctor-refuses-a-fifo");
    sandbox.write_config(
        "[plugins.macos-banner]
enabled = true
",
    );
    let state = sandbox.state();
    std::fs::create_dir_all(&state).expect("the state directory");
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(state.join("daemon-heartbeat"))
            .status()
            .is_ok_and(|status| status.success()),
        "the test needs a real FIFO"
    );

    let log = sandbox.path("doctor.out");
    let out = std::fs::File::create(&log).expect("the doctor log");
    let errors = out.try_clone().expect("the doctor log again");
    let mut child = sandbox
        .pns_stateful()
        .arg("doctor")
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(errors)
        .spawn()
        .expect("the engine runs");
    let finished = poll_until(|| child.try_wait().ok().flatten());
    if finished.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the doctor opened the FIFO and hung instead of reporting a state");
    }
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        said.contains("the daemon is enabled and has not run yet"),
        "a heartbeat that is not a file reads as no heartbeat: {said}"
    );
}

/// One hook run, payload on stdin, in `tests/hooks.rs`'s own shape.
fn hook(sandbox: &Sandbox, event: &str, payload: &str) -> std::process::Output {
    use std::io::Write;
    let mut child = sandbox
        .pns_stateful()
        .args(["hook", event])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload");
    child.wait_with_output().expect("output")
}
