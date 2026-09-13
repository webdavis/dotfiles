use super::*;
use crate::{CommandOutput, converge::tests::fixture::Scratch};
use std::{
    ffi::{OsStr, OsString},
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
};

#[derive(Default)]
struct Validation {
    args: Vec<OsString>,
    directory: Option<PathBuf>,
    mode: Option<u32>,
    result: Option<InspectionFailure>,
    refuse_cleanup: bool,
}
impl CommandRunner for Validation {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        _: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(program, Path::new("/fixture/sudo"));
        self.args = args.iter().map(|arg| arg.to_os_string()).collect();
        if args.len() == 7 && args[5] == "--database_path" {
            let db = PathBuf::from(args[6]);
            let parent = db.parent().unwrap();
            self.mode = Some(fs::metadata(parent).unwrap().permissions().mode() & 0o7777);
            self.directory = Some(parent.into());
            fs::create_dir(&db).unwrap();
            fs::write(db.join("LOCK"), b"owned fixture").unwrap();
            if self.refuse_cleanup {
                fs::set_permissions(&db, fs::Permissions::from_mode(0o000)).unwrap();
            }
        }
        match self.result {
            Some(InspectionFailure::Failed) => Ok(CommandOutput {
                bytes: vec![],
                exit: 1,
            }),
            Some(error) => Err(error),
            None => Ok(CommandOutput {
                bytes: vec![],
                exit: 0,
            }),
        }
    }
}

#[test]
fn daemon_validation_uses_a_private_database_and_cleans_it_after_every_result() {
    for result in [
        None,
        Some(InspectionFailure::Failed),
        Some(InspectionFailure::TimedOut),
    ] {
        let root = Scratch::new();
        let target = root.0.join("target with spaces");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("osquery.db"), b"live fixture remains unchanged").unwrap();
        let mut control = OsqueryRestart::new(
            Validation {
                result,
                ..Validation::default()
            },
            "/fixture/sudo".into(),
            "/trusted/osqueryctl".into(),
            Some("/trusted daemon/osqueryi".into()),
            target.clone(),
        );
        assert_eq!(control.config_check_in(&root.0), result.map_or(Ok(()), Err));
        let args = &control.runner.args;
        assert_eq!(
            args.len(),
            7,
            "validation did not use an isolated daemon database: {args:?}"
        );
        assert_eq!(
            &args[..6],
            [
                OsString::from("-n"),
                "/trusted daemon/osqueryi".into(),
                "--config_path".into(),
                target.join("osquery.conf").into_os_string(),
                "--config_check".into(),
                "--database_path".into()
            ]
        );
        let directory = control.runner.directory.as_ref().unwrap();
        assert_eq!(Path::new(&args[6]), directory.join("db"));
        assert!(directory.starts_with(&root.0));
        assert_eq!(control.runner.mode, Some(0o700));
        assert!(!directory.exists(), "validation database was retained");
        assert_eq!(
            fs::read(target.join("osquery.db")).unwrap(),
            b"live fixture remains unchanged"
        );
    }
}

#[test]
fn unavailable_private_database_refuses_before_any_command() {
    let root = Scratch::new();
    let mut control = OsqueryRestart::new(
        Validation::default(),
        "/fixture/sudo".into(),
        "/trusted/osqueryctl".into(),
        Some("/trusted/osqueryi".into()),
        root.0.clone(),
    );
    assert_eq!(
        control.config_check_in(&root.0.join("absent")),
        Err(InspectionFailure::Unavailable)
    );
    assert!(control.runner.args.is_empty());
}

#[test]
fn incomplete_database_cleanup_refuses_validation() {
    let root = Scratch::new();
    let mut control = OsqueryRestart::new(
        Validation {
            refuse_cleanup: true,
            ..Validation::default()
        },
        "/fixture/sudo".into(),
        "/trusted/osqueryctl".into(),
        Some("/trusted/osqueryi".into()),
        root.0.clone(),
    );
    let result = control.config_check_in(&root.0);
    let directory = control.runner.directory.as_ref().unwrap();
    let db = directory.join("db");
    assert!(fs::read_dir(&db).is_err(), "fixture must deny cleanup");
    fs::set_permissions(&db, fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir_all(directory).unwrap();
    assert_eq!(result, Err(InspectionFailure::Unavailable));
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
    let root = Scratch::new();
    fs::write(root.0.join("io.osquery.agent.plist"), b"vendor fixture").unwrap();
    let mut control = OsqueryRestart::new(
        Validation {
            result: Some(InspectionFailure::Failed),
            ..Validation::default()
        },
        "/fixture/sudo".into(),
        "/trusted/osqueryctl".into(),
        Some("/trusted/osqueryi".into()),
        root.0.clone(),
    );
    assert_eq!(
        restart_daemon(
            &mut control,
            &mut NoRestart,
            &mut NoRestart,
            RestartBounds::parse(None, None)
        ),
        Err(RestartFailure::Configuration(InspectionFailure::Failed))
    );
    assert_eq!(control.runner.args[4], "--config_check");
}
