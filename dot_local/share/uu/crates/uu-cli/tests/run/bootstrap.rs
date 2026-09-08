use super::*;

#[test]
fn bootstrap_of_a_lane_whose_type_has_no_bootstrap_step_is_refused_naming_the_type() {
    let home = Home::new("bootstrap-unsupported").with_herdr_lane(0);
    let output = home.uu(&["bootstrap", "herdr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("type `herdr` has no bootstrap step"),
        "{output:?}"
    );
}

#[test]
fn bootstrap_of_an_undeclared_lane_is_refused_like_a_run_of_one() {
    let home = Home::new("bootstrap-undeclared").with_herdr_lane(0);
    let output = home.uu(&["bootstrap", "hedr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("no `[lanes.hedr]` block"),
        "{output:?}"
    );
}

#[test]
fn bootstrap_of_a_configless_named_lane_is_refused_without_creating_history() {
    let home = Home::new("bootstrap-no-config");
    let output = home.uu(&["bootstrap", "herdr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("no config"), "{output:?}");
    assert!(!home.dir.join(".local/state/uu").exists());
}

#[test]
fn usage_lists_bootstrap_beside_run_doctor_and_schedule() {
    let home = Home::new("bootstrap-usage");
    let output = home.uu(&[]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        stderr(&output).contains("uu bootstrap <lane>"),
        "{output:?}"
    );
}
