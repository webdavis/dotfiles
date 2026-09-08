use super::*;

#[test]
fn completed_probes_retain_nonzero_status_and_ordered_output() {
    let mut runner = SystemRunner::new(Duration::from_millis(300));
    let result = runner.run_completed(
        Path::new("/bin/sh"),
        &[
            OsStr::new("-c"),
            OsStr::new("printf first; printf 'diagnostic\n' >&2; exit 1"),
        ],
        CommandIo::Inspection { merge_stderr: true },
    );
    assert_eq!(
        result,
        Ok(CommandOutput {
            bytes: b"firstdiagnostic\n".to_vec(),
            exit: 1
        })
    );
    let signalled = runner
        .run_completed(
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("printf stopped; kill -TERM $$"),
            ],
            CommandIo::Inspection { merge_stderr: true },
        )
        .unwrap();
    assert_eq!(
        signalled,
        CommandOutput {
            bytes: b"stopped".to_vec(),
            exit: 143
        }
    );
}

#[test]
fn a_poll_probe_gets_a_new_budget_after_the_previous_probe_times_out() {
    let mut runner = SystemRunner::per_command(Duration::from_millis(80));
    assert_eq!(
        runner.run_completed(
            Path::new("/bin/sh"),
            &[OsStr::new("-c"), OsStr::new("exec sleep 600")],
            CommandIo::Inspection {
                merge_stderr: false
            },
        ),
        Err(InspectionFailure::TimedOut),
    );
    assert_eq!(
        runner.run_completed(
            Path::new("/bin/sh"),
            &[OsStr::new("-c"), OsStr::new("printf next")],
            CommandIo::Inspection {
                merge_stderr: false
            },
        ),
        Ok(CommandOutput {
            bytes: b"next".to_vec(),
            exit: 0
        }),
    );
}

#[test]
fn a_total_inspection_budget_is_not_restarted_by_a_later_probe() {
    let mut runner = SystemRunner::new(Duration::from_millis(40));
    assert_eq!(
        runner.run(
            Path::new("/bin/sh"),
            &[OsStr::new("-c"), OsStr::new("exec sleep 600")],
            CommandIo::Inspection {
                merge_stderr: false
            }
        ),
        Err(InspectionFailure::TimedOut)
    );
    assert_eq!(
        runner.run(
            Path::new("/bin/sh"),
            &[OsStr::new("-c"), OsStr::new("printf later")],
            CommandIo::Inspection {
                merge_stderr: false
            }
        ),
        Err(InspectionFailure::TimedOut)
    );
}
