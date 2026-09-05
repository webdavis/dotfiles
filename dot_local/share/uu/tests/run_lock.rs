//! One run at a time, and the lane state a run no longer declares.

mod support;

use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use support::*;

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
