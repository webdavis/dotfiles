use super::*;
use crate::converge::tests::fixture::{Script, call, scratch};

fn daemon_control(target: PathBuf) -> OsqueryRestart<Script> {
    OsqueryRestart::new(
        Script::default(),
        "/fixture/sudo".into(),
        "/trusted/osqueryctl".into(),
        Some("/trusted daemon/osqueryi".into()),
        target,
    )
}

#[test]
fn the_daemon_configuration_check_opens_no_database_of_any_kind() {
    let mut control = daemon_control("/fixture/target with spaces".into());
    assert_eq!(control.config_check(), Ok(()));
    assert_eq!(
        control.runner.calls,
        [call(
            "/fixture/sudo",
            &[
                "-n",
                "/trusted daemon/osqueryi",
                "--config_path",
                "/fixture/target with spaces/osquery.conf",
                "--config_check",
                "--disable_database"
            ],
            CommandIo::Inspection {
                merge_stderr: false
            }
        )]
    );
}

#[test]
fn a_rejected_or_unreachable_daemon_check_reports_its_verdict_to_the_caller() {
    let mut control = daemon_control("/fixture/target".into());
    control.runner.exit = 1;
    assert_eq!(control.config_check(), Err(InspectionFailure::Failed));
    control.runner.failure = Some(InspectionFailure::TimedOut);
    assert_eq!(control.config_check(), Err(InspectionFailure::TimedOut));
    assert_eq!(control.runner.calls.len(), 2);
}

#[test]
fn rejected_daemon_validation_never_reaches_process_probes_or_stop() {
    use posture_application::{ProcessTable, RestartClock, RestartFailure, restart_daemon};
    use posture_domain::{ParentPid, RestartBounds};
    use std::time::Duration;
    struct NoRestart;
    impl ProcessTable for NoRestart {
        fn daemon_parent(&mut self) -> Result<Option<ParentPid>, InspectionFailure> {
            panic!("rejected validation must leave the daemon untouched")
        }
    }
    impl RestartClock for NoRestart {
        fn elapsed(&self) -> Duration {
            panic!("no restart")
        }
        fn sleep(&mut self, _: Duration) {
            panic!("no restart")
        }
    }
    let root = scratch();
    std::fs::write(
        root.path().join("io.osquery.agent.plist"),
        b"vendor fixture",
    )
    .unwrap();
    let mut control = daemon_control(root.path().to_path_buf());
    control.runner.exit = 1;
    assert_eq!(
        restart_daemon(
            &mut control,
            &mut NoRestart,
            &mut NoRestart,
            RestartBounds::parse(None, None)
        ),
        Err(RestartFailure::Configuration(InspectionFailure::Failed))
    );
    assert_eq!(control.runner.calls[0].args[4], "--config_check");
}
