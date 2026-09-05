//! The staleness bound: a lane that has gone quiet says so, exactly once per streak.

mod support;

use support::*;

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
