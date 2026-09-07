use super::*;

#[test]
fn a_staleness_alert_reads_the_previous_streak_before_the_new_count_is_published() {
    let home = Home::new("streak-alert-before-write");
    let updater = home.write_stub("updater", "cat >/dev/null\nexit 75\n");
    let engine = home.write_stub(
        "pns-stub",
        "cat \"$HOME/.local/state/uu/lanes/mine/streak\" >\"$HOME/alert-saw-streak\"\n",
    );
    let home = home.with_config(&format!(
        "[lanes.mine]\ntype = \"command\"\nrun = [\"{}\"]\n\n\
         [alerts]\nbinary = \"{}\"\n",
        updater.display(),
        engine.display(),
    ));
    for _ in 0..3 {
        let output = home.uu(&["run"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }
    assert_eq!(
        std::fs::read_to_string(home.dir.join("alert-saw-streak")).unwrap(),
        "2\n",
        "the trip must be delivered before its new count is published"
    );
    assert_eq!(
        std::fs::read_to_string(home.dir.join(".local/state/uu/lanes/mine/streak")).unwrap(),
        "3\n",
        "a delivered trip must then advance its count"
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
