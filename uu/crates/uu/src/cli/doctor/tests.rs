use super::*;

/// Plain, because these assert on wording and layout rather than on colour.
/// The escape sequences have their own tests in `style`.
const PLAIN: Paint = Paint::Plain;

#[test]
fn a_config_with_no_lanes_says_so_instead_of_printing_an_empty_section() {
    // An empty section reads as output that failed rather than as nothing to
    // report, and the two mean opposite things to someone debugging a run.
    assert_eq!(
        lanes(PLAIN, &Config::default()),
        ["  · none declared".to_string()]
    );
}

#[test]
fn delivery_names_both_halves_even_when_both_are_off() {
    // Silence about records would read as "records are fine".
    let rows = delivery(PLAIN, &Config::default());
    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("records are off"), "{rows:?}");
    assert!(rows[1].contains("alerts are off"), "{rows:?}");
}

#[test]
fn every_delivery_row_is_indented_under_its_section() {
    for row in delivery(PLAIN, &Config::default()) {
        assert!(row.starts_with("  · "), "{row:?}");
    }
}

#[test]
fn a_program_on_path_is_reported_with_where_it_was_found() {
    let described = describe_program("/bin/sh", false);
    // An absolute program that resolves to itself is printed once, not twice.
    assert_eq!(described, "/bin/sh, found");
}

#[test]
fn a_program_that_is_missing_says_the_lane_will_fail_every_week() {
    let described = describe_program("/nonexistent/updater", false);
    assert!(described.contains("NOT FOUND"), "{described}");
    assert!(described.contains("every scheduled run"), "{described}");
    assert!(described.contains("[alerts]"), "{described}");
}

#[test]
fn a_missing_program_names_the_webhook_when_one_is_configured() {
    // Same failure, different consequence: with a webhook the operator hears
    // about it, so telling them to configure [alerts] would be wrong.
    let described = describe_program("/nonexistent/updater", true);
    assert!(
        described.contains("failure webhook is configured"),
        "{described}"
    );
    assert!(!described.contains("[alerts]"), "{described}");
}

#[test]
fn a_relative_program_is_refused_rather_than_resolved() {
    // Resolving it would answer from wherever the operator is standing, which
    // says nothing about a run that starts in /.
    let described = describe_program("./updater", false);
    assert!(described.contains("RELATIVE PATH"), "{described}");
    assert!(!described.contains("found at"), "{described}");
}

#[test]
fn a_program_line_does_not_repeat_the_lane_name_above_it() {
    // It prints directly under the lane's own row, so naming the lane again
    // put it twice in two adjacent lines.
    assert!(!describe_program("/bin/sh", false).contains("lane"));
}

#[test]
fn the_schedule_section_reports_the_day_the_time_and_the_last_run() {
    let rows = schedule(PLAIN, &Config::default(), "/nonexistent");
    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("weekday"), "{rows:?}");
    assert!(rows[0].contains("uu schedule render"), "{rows:?}");
    assert!(rows.iter().all(|row| row.starts_with("  · ")), "{rows:?}");
}
