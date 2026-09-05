//! `uu doctor`, and the usage an unknown command gets.

mod support;

use support::*;

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
