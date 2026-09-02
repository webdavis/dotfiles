//! The run wiring, pinned end to end: exit codes, the marker policy and the
//! doctor's key hygiene are decided in main, where no unit test reaches, and a
//! mutation there (the marker written on a failed run, a lane failure leaking
//! into the exit code, a configless machine exiting non-zero) survived the
//! whole unit suite. Each test here spawns the real binary once against its
//! own scratch HOME, with the lane pointed at a stub herdr by absolute path so
//! nothing touches PATH, the network or the machine's own config.

use std::cell::Cell;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Instant;

/// The same speed guard as pns's `Sandbox` (`dot_local/share/pns/tests/support/mod.rs`):
/// two crates, no shared dev crate, so this is a deliberate duplicate rather
/// than an import. See that file for the full reasoning behind the two
/// numbers; in short, `TEST_BUDGET_MS` is the review line (`Drop` warns on
/// stderr, greppable as "test budget", and keeps going) and `TEST_CEILING_MS`
/// is the failure line, calibrated so parallel-scheduler contention never
/// reaches it.
const TEST_BUDGET_MS: u128 = 1_000;
const TEST_CEILING_MS: u128 = 5_000;

fn over_budget(elapsed_ms: u128) -> bool {
    elapsed_ms > TEST_BUDGET_MS
}

fn over_ceiling(elapsed_ms: u128, excused: bool, panicking: bool) -> bool {
    elapsed_ms > TEST_CEILING_MS && !excused && !panicking
}

struct Home {
    dir: PathBuf,
    created: Instant,
    excused: Cell<bool>,
}

impl Home {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("uu-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch HOME");
        Home {
            dir,
            created: Instant::now(),
            excused: Cell::new(false),
        }
    }

    /// Excuse THIS home from `TEST_CEILING_MS` because its cost is
    /// structural rather than a regression. `&self`: tests hold their home
    /// immutably, so the excuse is a `Cell`. Never silences the WARNING at
    /// `TEST_BUDGET_MS`, only the failure at the ceiling.
    fn allow_slow(&self, reason: &'static str) {
        debug_assert!(!reason.is_empty(), "allow_slow needs a real reason");
        self.excused.set(true);
    }

    fn with_config(self, text: &str) -> Self {
        let config = self.dir.join(".config/uu");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::write(config.join("config.toml"), text).expect("config file");
        self
    }

    /// A herdr that answers every call with `exit_code`, and a lane block
    /// pointing straight at it.
    fn with_herdr_lane(self, exit_code: i32) -> Self {
        self.with_herdr_lane_and(&format!("exit {exit_code}\n"), "")
    }

    /// The same lane, with `body` as the stub herdr's whole script and `extra`
    /// appended to the config file.
    fn with_herdr_lane_and(self, body: &str, extra: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let stub = self.dir.join("herdr-stub");
        std::fs::write(&stub, format!("#!/bin/sh\n{body}")).expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let text = format!("[lanes.herdr]\nbinary = \"{}\"\n{extra}", stub.display());
        self.with_config(&text)
    }

    /// An executable shell script written into the scratch HOME, for a
    /// command lane's `run` (or `[alerts]`'s `binary`) to point at.
    fn write_stub(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let stub = self.dir.join(name);
        std::fs::write(&stub, format!("#!/bin/sh\n{body}")).expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
        stub
    }

    fn uu(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_uu"))
            .args(args)
            .env("HOME", &self.dir)
            .output()
            .expect("spawn uu")
    }

    fn marker(&self) -> PathBuf {
        self.dir.join(".local/state/uu/last-success")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let elapsed = self.created.elapsed().as_millis();
        if over_budget(elapsed) {
            // THE PROCESS'S OWN STDERR, not `eprintln!`: libtest captures the
            // print macros of a passing test and shows them only on failure or
            // under `--show-output`, which would swallow the one line the
            // review rule exists to print.
            use std::io::Write as _;
            let _ = writeln!(
                std::io::stderr(),
                "test budget: home {:?} took {elapsed} ms, over the {TEST_BUDGET_MS} ms budget",
                self.dir
            );
        }
        if over_ceiling(elapsed, self.excused.get(), std::thread::panicking()) {
            panic!(
                "test budget: home {:?} took {elapsed} ms, over the {TEST_CEILING_MS} ms ceiling \
                 (call allow_slow(\"reason\") if this is structural)",
                self.dir
            );
        }
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A loopback port with nothing behind it: bound only to learn a number the
/// kernel says is free, then released. A connection there is REFUSED at once,
/// so the record path's failure arm is exercised without waiting out a
/// deadline or reaching the network.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    listener.local_addr().expect("its number").port()
}

/// Accept exactly one connection, read the WHOLE request (headers plus the
/// Content-Length body), answer 200, and hand back the body as text.
///
/// Mirrors the raw-HTTP read pns's own hermes tests use
/// (`dot_local/share/pns/src/channels/hermes.rs`, the redirect test): two
/// crates, no shared dev dependency, so this is a deliberate duplicate rather
/// than an import. A response after only a partial read can reset the socket
/// under a client still writing, which is why this drains to Content-Length
/// before answering.
fn read_posted_body(listener: std::net::TcpListener) -> String {
    use std::io::{Read, Write};
    let (mut stream, _) = listener.accept().expect("the uu binary's POST");
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break raw.len();
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(position) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let content_length = String::from_utf8_lossy(&raw[..header_end])
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    while raw.len() < header_end + content_length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
    let _ = stream.flush();
    String::from_utf8_lossy(&raw[header_end..header_end + content_length]).to_string()
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

// --- the run lock -------------------------------------------------------

/// The run lock's own path, matching `run_lock_path` in `main.rs`
/// (duplicated rather than exported: a lock path only a test needs is not
/// part of the binary's public surface).
fn lock_path(home: &std::path::Path) -> PathBuf {
    home.join(".local/state/uu/run.lock")
}

#[test]
fn a_second_run_finding_the_lock_already_held_refuses_and_exits_rather_than_racing() {
    // THE RACE THIS CLOSES (reproduced by hand before this fix): two
    // concurrent `uu run` invocations against the same HOME both read a
    // lane's streak as 0 and both wrote 1, silently losing a count that
    // should have reached 2. Holding the lock from THIS test, rather than
    // spawning a second real `uu`, is what makes the refusal deterministic:
    // a genuine race depends on timing this test would otherwise have to
    // fight to land inside budget.
    let home = Home::new("lock-held");
    let stub = home.write_stub("updater", "cat >/dev/null\nexit 0\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    let lock_path = lock_path(&home.dir);
    std::fs::create_dir_all(lock_path.parent().unwrap()).expect("lock dir");
    let held = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lock_path)
        .expect("open the lock file");
    // SAFETY: `held` stays open for the rest of this test, so the kernel
    // keeps the lock until it is dropped at the end of the function.
    let refused = unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(refused, 0, "the test itself must be able to take the lock");

    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("already holds"),
        "a refused run must say so rather than fail silently: {output:?}"
    );
    assert!(
        !home.marker().exists(),
        "a run refused the lock must not pretend it ran: {output:?}"
    );
    drop(held);
}

#[test]
fn a_run_that_gets_the_lock_leaves_nothing_for_the_next_run_to_trip_on() {
    // The lock is dropped at the end of `run_mode`, not held for the whole
    // process: a normal exit must free it immediately, or every run after
    // the first would refuse itself.
    let home = Home::new("lock-released").with_herdr_lane(0);
    assert_eq!(home.uu(&["run"]).status.code(), Some(0));
    assert_eq!(home.uu(&["run"]).status.code(), Some(0));
}

#[test]
fn a_lock_that_could_not_even_be_opened_names_its_own_cause_not_contention() {
    // FINDING 3 (6v): "not running, to avoid racing the run that already
    // holds it" is only true for the `flock` refusal above. Making the state
    // directory unwritable hits a DIFFERENT arm, the `OpenOptions::open`
    // failure inside `acquire_run_lock`, which the review reproduced with
    // `Permission denied (os error 13)`; the operator reading that line needs
    // the real cause, not a claim about a race that never happened.
    let home = Home::new("lock-unopenable").with_herdr_lane(0);
    let state_dir = home.dir.join(".local/state/uu");
    std::fs::create_dir_all(&state_dir).expect("state dir");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&state_dir, std::fs::Permissions::from_mode(0o555))
        .expect("make the state directory unwritable");
    let output = home.uu(&["run"]);
    // Restore write access before any assertion below can panic, or Home's
    // `Drop` cannot clean up its own scratch directory.
    std::fs::set_permissions(&state_dir, std::fs::Permissions::from_mode(0o755))
        .expect("restore the state directory so cleanup can run");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = stderr(&output);
    assert!(
        message.contains("could not open"),
        "the real cause must be named: {message}"
    );
    assert!(
        !message.contains("already holds") && !message.contains("avoid racing"),
        "a permission failure is not contention, and must not be described as one: {message}"
    );
}

// --- pruning a lane the config no longer declares -----------------------

#[test]
fn a_lane_removed_from_the_config_has_its_old_streak_directory_pruned() {
    let home = Home::new("prune-removed-lane");
    let gone = home.dir.join(".local/state/uu/lanes/gone");
    std::fs::create_dir_all(&gone).expect("a stale lane directory");
    std::fs::write(gone.join("streak"), "2\n").expect("its old streak");
    let stub = home.write_stub("updater", "cat >/dev/null\nexit 0\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        !gone.exists(),
        "a lane no longer declared must not keep its directory forever: {output:?}"
    );
}

#[test]
fn a_lane_still_declared_keeps_its_streak_directory_across_a_run() {
    let home = Home::new("prune-keeps-current");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    home.uu(&["run"]);
    let mine = home.dir.join(".local/state/uu/lanes/mine");
    assert_eq!(
        std::fs::read_to_string(mine.join("streak")).unwrap().trim(),
        "1",
        "the still-declared lane's own streak must survive pruning"
    );
}

#[test]
fn a_new_lane_reusing_a_pruned_names_directory_never_inherits_its_old_streak() {
    let home = Home::new("prune-then-reuse");
    let old = home.dir.join(".local/state/uu/lanes/mine");
    std::fs::create_dir_all(&old).expect("a stale lane directory");
    std::fs::write(old.join("streak"), "2\n").expect("its old streak, one short of the threshold");
    // A run with no `mine` lane declared prunes the old directory.
    let elsewhere = home.write_stub("elsewhere-updater", "exit 0\n");
    let home = home.with_config(&format!(
        "[lanes.elsewhere]\ntype = \"command\"\nrun = [\"{}\"]\n",
        elsewhere.display()
    ));
    home.uu(&["run"]);
    assert!(!old.exists(), "{old:?} must have been pruned already");
    // A NEW "mine" lane, one run later, must start its own streak at zero:
    // deferring once here must not read back the old lane's "2" and trip the
    // staleness alert on what is really its first ever miss.
    let deferring = home.write_stub(
        "deferring-updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        deferring.display(),
        pns_stub.display(),
    ));
    home.uu(&["run"]);
    assert!(
        !home.dir.join("alert-args").exists(),
        "a lane reusing a pruned name must not alert on its very first miss"
    );
}

// --- the staleness bound -----------------------------------------------------

#[test]
fn a_lane_deferring_stale_after_runs_times_in_a_row_fires_one_staleness_alert() {
    let home = Home::new("stale-trip");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    // Two deferrals: below the threshold, so no staleness alert yet (only the
    // per-run failure alert would fire, and a deferral never does that
    // either).
    home.uu(&["run"]);
    home.uu(&["run"]);
    assert!(
        !home.dir.join("alert-args").exists(),
        "the staleness alert must not fire before the threshold"
    );
    // The third deferral crosses it.
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert!(alerts.contains("mine"), "{alerts}");
    assert!(alerts.contains("3 consecutive"), "{alerts}");
}

#[test]
fn the_staleness_alert_fires_once_at_the_threshold_and_not_again_while_still_deferring() {
    let home = Home::new("stale-once");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    for _ in 0..3 {
        home.uu(&["run"]);
    }
    let after_third = std::fs::read_to_string(home.dir.join("alert-args")).expect("alert args");
    let times_after_third = after_third.matches("mine").count();
    assert_eq!(times_after_third, 1, "{after_third}");
    home.uu(&["run"]);
    let after_fourth = std::fs::read_to_string(home.dir.join("alert-args")).expect("alert args");
    assert_eq!(
        after_fourth.matches("mine").count(),
        1,
        "a fourth straight deferral must not alert again: {after_fourth}"
    );
}

#[test]
fn a_lane_failing_stale_after_runs_times_in_a_row_also_fires_one_staleness_alert() {
    // Every staleness test above uses a DEFERRING lane. A mutant reading
    // `succeeded` as `!report.deferred` (dropping the `&& failures == 0`
    // half) would treat a plain FAILURE as a success, since a failure never
    // sets `deferred`, resetting the streak every run and never tripping.
    // Only a lane that actually FAILS three times in a row can catch that.
    let home = Home::new("stale-trip-failures");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'boom: disk full\\n' >&2\nexit 2\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    for _ in 0..3 {
        home.uu(&["run"]);
    }
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert_eq!(
        alerts.matches("consecutive").count(),
        1,
        "three straight failures must trip the staleness alert exactly once: {alerts}"
    );
}

#[test]
fn two_lanes_deferring_together_trip_their_own_staleness_alert_independently() {
    // Every staleness test above uses ONE lane. A mutant sharing one streak
    // path across every lane (dropping the lane's name from `streak_path`)
    // would have the second lane inherit the first's count: run in NAME
    // order (alpha before beta), that would trip alpha on run 2 instead of
    // run 3, and beta would never trip on its own account at all.
    let home = Home::new("stale-two-lanes");
    let stub_a = home.write_stub(
        "updater-alpha",
        "cat >/dev/null\nprintf 'alpha deferred\\n' >&2\nexit 75\n",
    );
    let stub_b = home.write_stub(
        "updater-beta",
        "cat >/dev/null\nprintf 'beta deferred\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.alpha]\ntype = \"command\"\nrun = [\"{}\"]\n\n\
         [lanes.beta]\ntype = \"command\"\nrun = [\"{}\"]\n\n\
         [alerts]\nbinary = \"{}\"\n",
        stub_a.display(),
        stub_b.display(),
        pns_stub.display(),
    ));
    for _ in 0..3 {
        home.uu(&["run"]);
    }
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert_eq!(
        alerts.matches("alpha").count(),
        1,
        "alpha must trip exactly once, on its own third run: {alerts}"
    );
    assert_eq!(
        alerts.matches("beta").count(),
        1,
        "beta must trip exactly once too, independently of alpha: {alerts}"
    );
}

#[test]
fn a_staleness_alert_the_engine_refused_is_retried_rather_than_lost_for_good() {
    // THE HOLE THE STALENESS BOUND EXISTS TO CLOSE, REOPENED. It fires once
    // per streak, so an engine that was down for that ONE run leaves a
    // deferring lane with no alert at all until a success it may never have:
    // a deferral raises nothing else, so the lane is silent for good.
    let home = Home::new("stale-engine-down");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'lock held\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub(
        "pns-stub",
        "[ -f \"$HOME/engine-down\" ] && exit 1\nprintf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n",
    );
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    std::fs::write(home.dir.join("engine-down"), "").expect("the engine is down");
    for _ in 0..3 {
        home.uu(&["run"]);
    }
    assert!(
        !home.dir.join("alert-args").exists(),
        "the refusing engine delivered nothing, which is the premise"
    );
    // The engine is back. The lane is still deferring and has still not
    // succeeded, so the alert it owes is still owed.
    std::fs::remove_file(home.dir.join("engine-down")).expect("the engine is back");
    home.uu(&["run"]);
    let alerts = std::fs::read_to_string(home.dir.join("alert-args"))
        .expect("the staleness alert must be retried once the engine answers again");
    assert!(alerts.contains("mine"), "{alerts}");
    assert_eq!(
        alerts.matches("consecutive").count(),
        1,
        "the retry is still exactly one alert: {alerts}"
    );
}

#[test]
fn a_success_between_deferrals_resets_the_staleness_streak() {
    let home = Home::new("stale-reset");
    let counter = home.dir.join("call-count");
    let stub = home.write_stub(
        "updater",
        &format!(
            "cat >/dev/null\n\
             count=$(( $(cat {counter:?} 2>/dev/null || printf 0) + 1 ))\n\
             printf '%s' \"$count\" >{counter:?}\n\
             if [ \"$count\" -eq 2 ]; then exit 0; fi\n\
             printf 'nothing was attempted\\n' >&2\n\
             exit 75\n",
        ),
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    // defer, succeed (resets the streak to zero), defer, defer: never three
    // in a row, so the staleness alert must never fire.
    for _ in 0..4 {
        home.uu(&["run"]);
    }
    assert!(
        !home.dir.join("alert-args").exists(),
        "a success in the middle must reset the streak, so three total \
         deferrals spread across a reset must not trip the alert"
    );
}

#[test]
fn an_unreadable_streak_is_treated_as_already_close_to_stale_not_reset_to_zero() {
    // FINDING 2 (6v): `read_streak`'s own unit tests pin what `Unreadable`
    // IS, but nothing pinned what `run_mode` DOES with it. The deliberate
    // choice there is to seed the streak one short of the threshold rather
    // than starting fresh at zero, so a file that briefly held garbage does
    // not quietly forgive whatever real streak it was tracking. One more
    // deferral on top of a garbage streak file must be enough to trip the
    // staleness alert; starting over at zero would need two more.
    let home = Home::new("streak-unreadable");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    let streak_file = home.dir.join(".local/state/uu/lanes/mine/streak");
    std::fs::create_dir_all(streak_file.parent().unwrap()).expect("streak dir");
    std::fs::write(&streak_file, "not-a-number\n").expect("a garbage streak value");
    home.uu(&["run"]);
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert!(
        alerts.contains("consecutive"),
        "one more deferral after an unreadable streak must trip the staleness \
         alert, not reset the count to zero: {alerts}"
    );
}

// --- the streak file's own I/O -----------------------------------------------

#[test]
fn an_unwritable_streak_directory_alerts_instead_of_staying_silent_forever() {
    // ROW 2, DIRECTION A, reproduced by hand before this fix: a plain file
    // sitting where the lane's own directory belongs makes every write fail,
    // and before this fix that failure only reached stderr, so a lane stuck
    // this way never once reached the staleness threshold no matter how many
    // times it deferred.
    let home = Home::new("streak-unwritable-dir");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    std::fs::create_dir_all(home.dir.join(".local/state/uu")).expect("state dir");
    std::fs::write(home.dir.join(".local/state/uu/lanes"), "")
        .expect("occupy the lanes path with a plain file");
    for _ in 0..4 {
        home.uu(&["run"]);
    }
    let alerts = std::fs::read_to_string(home.dir.join("alert-args"))
        .expect("an unwritable streak directory must be reported loudly, not stay silent");
    assert!(alerts.contains("could not be recorded"), "{alerts}");
}

#[test]
fn a_streak_file_made_read_only_between_runs_is_still_correctly_advanced() {
    // ROW 2, DIRECTION B, reproduced by hand before this fix: once the
    // streak file itself could not be written, the persisted value never
    // advanced past whatever first crossed the threshold, so every run after
    // it re-tripped the identical staleness alert forever. Publishing the
    // streak by rename fixes this directly: a rename only needs write
    // permission on the DIRECTORY, so a read-only streak FILE no longer
    // blocks anything.
    let home = Home::new("streak-readonly-file");
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'nothing was attempted\\n' >&2\nexit 75\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >>\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    home.uu(&["run"]);
    home.uu(&["run"]);
    let streak_file = home.dir.join(".local/state/uu/lanes/mine/streak");
    assert_eq!(std::fs::read_to_string(&streak_file).unwrap().trim(), "2");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&streak_file, std::fs::Permissions::from_mode(0o444))
        .expect("make the streak file read-only");
    // The third deferral must still cross the threshold and persist past it,
    // exactly as it would have with a writable file.
    home.uu(&["run"]);
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert_eq!(alerts.matches("consecutive").count(), 1, "{alerts}");
    assert_eq!(
        std::fs::read_to_string(&streak_file).unwrap().trim(),
        "3",
        "a read-only file must not block the streak from actually advancing"
    );
    // A fourth deferral must not re-trip: the value truly advanced, so this
    // is no longer the run that first crosses the threshold.
    home.uu(&["run"]);
    let alerts = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert_eq!(
        alerts.matches("consecutive").count(),
        1,
        "a fourth straight deferral must not alert again now that persistence works: {alerts}"
    );
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

// --- the command lane -------------------------------------------------------

#[test]
fn a_command_lane_runs_end_to_end_and_the_record_carries_what_it_printed() {
    let home = Home::new("command-lane");
    let stub = home.write_stub("updater", "cat >\"$HOME/event\"; printf '3 upgraded\\n'\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        stub.display()
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let event = std::fs::read_to_string(home.dir.join("event")).expect("the event file");
    assert!(event.contains("\"lane\":\"mine\""), "{event}");
    assert!(
        stdout(&output).contains("3 upgraded"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_failed_command_lane_alerts_through_the_configured_engine() {
    let home = Home::new("command-lane-failed");
    // Both stdout and stderr, and a non-1 exit: a fixture that says nothing
    // on either stream passes even when the exit code or the stderr tail
    // never reach the record or the alert.
    let stub = home.write_stub(
        "updater",
        "cat >/dev/null\nprintf 'did some upgrading\\n'\nprintf 'boom: disk full\\n' >&2\nexit 2\n",
    );
    let pns_stub = home.write_stub("pns-stub", "printf '%s\\n' \"$*\" >\"$HOME/alert-args\"\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n[alerts]\nbinary = \"{}\"\n",
        stub.display(),
        pns_stub.display(),
    ));
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let record = stdout(&output);
    assert!(record.contains("did some upgrading"), "{record}");
    assert!(record.contains("exit 2"), "{record}");
    assert!(record.contains("boom: disk full"), "{record}");
    let args = std::fs::read_to_string(home.dir.join("alert-args")).expect("the alert args");
    assert!(args.contains("--state failed"), "{args}");
    // The lane's own NAME heads the detail, not its type and not a path that
    // happens to hold the word, and the exit code and stderr tail ride along
    // with it rather than a bare failure count.
    assert!(args.contains("--detail mine: 1 failure(s);"), "{args}");
    assert!(args.contains("exit 2"), "{args}");
    assert!(args.contains("boom: disk full"), "{args}");
}

#[test]
fn the_doctor_lists_a_command_lane_with_its_program_resolved() {
    let home = Home::new("command-lane-doctor");
    let stub = home.write_stub("updater", "exit 0\n");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\", \"--yes\"]\n",
        stub.display()
    ));
    let output = home.uu(&["doctor"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("lane mine: on (command)"),
        "{output:?}"
    );
    assert!(
        stdout(&output).contains(&format!("found at {}", stub.display())),
        "{output:?}"
    );
}

#[test]
fn the_doctor_says_a_missing_program_will_fail_weekly_and_alert_only_if_configured() {
    let home = Home::new("command-lane-doctor-missing");
    let missing = home.dir.join("no-such-updater");
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n",
        missing.display()
    ));
    let output = home.uu(&["doctor"]);
    // Doctor REPORTS, it does not refuse: a lane whose program is missing is
    // a finding on the way to the weekly run, not a reason to stop looking.
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let out = stdout(&output);
    assert!(out.contains("lane mine: on (command)"), "{out}");
    assert!(
        out.contains(
            "NOT FOUND; every scheduled run of this lane will fail, and it alerts only when \
             [alerts] is configured"
        ),
        "{out}"
    );
    assert!(
        out.contains("the weekly run uses the plist's own PATH"),
        "{out}"
    );
}

#[test]
fn the_doctor_flags_a_relative_command_path_as_resolving_differently_under_the_weekly_run() {
    // Doctor runs from wherever the operator's shell happens to be; the
    // weekly launchd job starts at `/`. `resolve` would answer `found` or
    // `NOT FOUND` for `./nothing-here` from doctor's own cwd, which says
    // nothing about what the weekly run at `/` will see.
    let home = Home::new("command-lane-doctor-relative");
    let home = home.with_config("[lanes.mine]\ntype = \"command\"\nrun = [\"./nothing-here\"]\n");
    let output = home.uu(&["doctor"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let out = stdout(&output);
    assert!(out.contains("lane mine: on (command)"), "{out}");
    assert!(
        out.contains(
            "RELATIVE PATH; the weekly run starts in /, so this resolves differently there"
        ),
        "{out}"
    );
}

/// The epoch at the head of a marker or a stub's breadcrumb.
fn epoch_in(text: &str) -> i64 {
    text.split_whitespace()
        .next()
        .expect("an epoch field")
        .parse()
        .expect("an epoch")
}

#[test]
fn the_doctor_never_prints_the_records_signing_key() {
    let home = Home::new("doctor").with_config("[records]\nkey = \"s3cr3t-value\"\n");
    let output = home.uu(&["doctor"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let everything = format!(
        "{}{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!everything.contains("s3cr3t-value"), "{everything}");
    assert!(everything.contains("key set"), "{everything}");
}

#[test]
fn the_doctor_lists_each_declared_lane_with_its_type() {
    let home = Home::new("doctor-lanes").with_config("[lanes.mine]\ntype = \"herdr\"\n");
    let output = home.uu(&["doctor"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let out = stdout(&output);
    assert!(out.contains("lane mine: on (herdr)"), "{output:?}");
    assert!(!out.contains("none declared"), "{output:?}");
}

#[test]
fn the_doctor_says_so_when_the_config_declares_no_lane() {
    let home = Home::new("doctor-no-lanes").with_config("[schedule]\nday = \"sunday\"\n");
    let output = home.uu(&["doctor"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("lanes: none declared"),
        "{output:?}"
    );
}

#[test]
fn an_unknown_command_is_usage_on_stderr_and_exit_two() {
    let home = Home::new("usage");
    let output = home.uu(&["bogus"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("usage"), "{output:?}");
    // The line's SHAPE, not its exact text: a build that adds a lane type
    // lengthens the list, and the pin here is that usage lists the types at
    // all and names every one this build serves, not just `herdr`.
    let types: Vec<&str> = err
        .lines()
        .find_map(|line| line.strip_prefix("lane types: "))
        .map(|types| types.split(", ").collect())
        .unwrap_or_default();
    assert!(types.contains(&"command"), "{output:?}");
    assert!(types.contains(&"herdr"), "{output:?}");
}

#[cfg(test)]
mod guard_tests {
    //! The same twins as pns's copy of this guard: the pure predicates are
    //! pinned with literal inputs rather than the constants they check, and
    //! the two end to end tests backdate a real `Home`'s construction
    //! instant instead of sleeping past it. The two backdated twins each
    //! print one budget line to stderr on every run, by construction; the
    //! home name in that line says "guard-twin".
    use super::*;

    #[test]
    fn a_fast_home_is_not_over_budget() {
        assert!(!over_budget(10));
    }

    #[test]
    fn a_home_past_the_budget_is_over_budget() {
        assert!(over_budget(1_500));
    }

    #[test]
    fn a_home_past_the_ceiling_with_no_excuse_is_over_ceiling() {
        assert!(over_ceiling(6_000, false, false));
    }

    #[test]
    fn an_excused_home_is_never_over_ceiling() {
        assert!(!over_ceiling(6_000, true, false));
    }

    #[test]
    fn an_already_panicking_thread_is_never_double_panicked() {
        assert!(!over_ceiling(6_000, false, true));
    }

    /// Drop a real home whose construction instant was pushed back by
    /// `age_ms`, optionally excused, and say what its own drop panicked
    /// with, if anything.
    fn drop_backdated(name: &str, age_ms: u64, excuse: Option<&'static str>) -> Option<String> {
        let mut home = Home::new(name);
        home.created = Instant::now() - std::time::Duration::from_millis(age_ms);
        if let Some(reason) = excuse {
            home.allow_slow(reason);
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(home)))
            .err()
            .map(|payload| {
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_default()
            })
    }

    #[test]
    fn a_real_home_past_the_ceiling_fails_naming_the_test_budget() {
        let message = drop_backdated("guard-twin-ceiling", TEST_CEILING_MS as u64 + 1, None)
            .expect("a home over the ceiling must fail its own drop");
        assert!(message.starts_with("test budget:"), "{message}");
    }

    #[test]
    fn a_real_home_past_the_ceiling_with_allow_slow_does_not_fail() {
        assert!(
            drop_backdated(
                "guard-twin-ceiling-excused",
                TEST_CEILING_MS as u64 + 1,
                Some("a structural reason, for this twin alone")
            )
            .is_none(),
            "allow_slow must lift the ceiling"
        );
    }
}
