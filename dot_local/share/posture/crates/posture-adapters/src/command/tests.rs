use super::*;

#[test]
fn stdout_and_stderr_share_the_original_write_order() {
    let mut runner = SystemRunner::new(Duration::from_millis(300));
    let output = runner.run(
        Path::new("/bin/sh"),
        &[
            OsStr::new("-c"),
            OsStr::new("printf first; printf second >&2; printf third"),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    assert_eq!(output, Ok(b"firstsecondthird".to_vec()));
}

#[test]
fn separate_stderr_is_discarded_and_trailing_newlines_are_retained_by_runner() {
    let mut runner = SystemRunner::new(Duration::from_millis(300));
    let output = runner.run(
        Path::new("/bin/sh"),
        &[
            OsStr::new("-c"),
            OsStr::new("printf 'facts\\n\\n'; printf diagnostic >&2"),
        ],
        CommandIo::Inspection {
            merge_stderr: false,
        },
    );
    assert_eq!(output, Ok(b"facts\n\n".to_vec()));
}

#[test]
fn a_failed_child_does_not_supply_a_successful_reading() {
    let mut runner = SystemRunner::new(Duration::from_millis(300));
    assert_eq!(
        runner.run(
            Path::new("/bin/sh"),
            &[OsStr::new("-c"), OsStr::new("printf misleading; exit 5")],
            CommandIo::Inspection {
                merge_stderr: false
            }
        ),
        Err(InspectionFailure::Failed)
    );
}

#[test]
fn a_missing_executable_is_unavailable() {
    let mut runner = SystemRunner::new(Duration::from_millis(300));
    assert_eq!(
        runner.run(
            Path::new("/fixture/absent-enricher-probe"),
            &[],
            CommandIo::Inspection {
                merge_stderr: false
            }
        ),
        Err(InspectionFailure::Unavailable)
    );
}

#[test]
fn an_exhausted_budget_never_starts_another_probe() {
    let mut runner = SystemRunner::new(Duration::ZERO);
    assert_eq!(
        runner.run(
            Path::new("/fixture/absent-enricher-probe"),
            &[],
            CommandIo::Inspection {
                merge_stderr: false
            }
        ),
        Err(InspectionFailure::TimedOut)
    );
}

mod lifecycle;

mod terminal;

mod outcomes;
