use super::*;

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
