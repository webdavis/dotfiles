use super::*;

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

const GENEROUS: Duration = Duration::from_secs(10);

#[test]
fn returns_what_the_command_printed() {
    let out = capture(&argv(&["/bin/echo", "one PR open"]), GENEROUS).unwrap();
    assert_eq!(out, "one PR open\n");
}

#[test]
fn a_command_that_is_not_installed_says_so_rather_than_failing_the_run() {
    let error = capture(&argv(&["morning-no-such-command"]), GENEROUS).unwrap_err();
    assert!(
        error.to_string().starts_with("could not run it:"),
        "{error}"
    );
}

#[test]
fn a_failing_command_reports_its_status_and_its_first_stderr_line() {
    let script = "echo 'gh: not authenticated' >&2; exit 4";
    let error = capture(&argv(&["/bin/sh", "-c", script]), GENEROUS).unwrap_err();
    assert_eq!(error.to_string(), "it exited 4: gh: not authenticated");
}

#[test]
fn a_command_that_outruns_its_deadline_is_killed_and_reported() {
    let started = Instant::now();
    let limit = Duration::from_millis(150);
    let error = capture(&argv(&["/bin/sh", "-c", "sleep 30"]), limit).unwrap_err();
    assert!(matches!(error, CaptureError::TimedOut(_)), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "waited {:?}",
        started.elapsed()
    );
}

#[test]
fn a_command_with_no_program_is_a_configuration_failure() {
    let error = capture(&[], GENEROUS).unwrap_err();
    assert_eq!(
        error.to_string(),
        "it was configured with no command to run"
    );
}
