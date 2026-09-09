use super::*;
use crate::converge::tests::fixture::*;

fn installer() -> ConvergeInstaller<Script> {
    ConvergeInstaller::new(
        Script::default(),
        "/fixture/sudo".into(),
        "/fixture/target".into(),
        "/fixture/log".into(),
    )
}

#[test]
fn test_the_install_carries_owner_group_and_mode_in_one_call() {
    let mut install = installer();
    assert_eq!(
        install.file(
            ConvergeFile::Configuration,
            Path::new("/private stage/osquery.conf")
        ),
        Ok(())
    );
    assert_eq!(
        install.runner.calls,
        [call(
            "/fixture/sudo",
            &[
                "-n",
                "/usr/bin/install",
                "-o",
                "root",
                "-g",
                "wheel",
                "-m",
                "0644",
                "/private stage/osquery.conf",
                "/fixture/target/osquery.conf"
            ],
            CommandIo::InheritAll
        )]
    );
}

#[test]
fn both_directory_targets_use_one_absolute_install_with_root_wheel_and_0755() {
    let mut install = installer();
    for directory in ConvergeDirectory::ALL {
        assert_eq!(install.directory(directory), Ok(()));
    }
    assert_eq!(
        install.runner.calls,
        [
            call(
                "/fixture/sudo",
                &[
                    "-n",
                    "/usr/bin/install",
                    "-d",
                    "-o",
                    "root",
                    "-g",
                    "wheel",
                    "-m",
                    "0755",
                    "/fixture/target"
                ],
                CommandIo::InheritAll
            ),
            call(
                "/fixture/sudo",
                &[
                    "-n",
                    "/usr/bin/install",
                    "-d",
                    "-o",
                    "root",
                    "-g",
                    "wheel",
                    "-m",
                    "0755",
                    "/fixture/target/packs"
                ],
                CommandIo::InheritAll
            )
        ]
    );
}

#[test]
fn unsuccessful_install_outcomes_remain_typed_failures() {
    let mut install = installer();
    install.runner.exit = 7;
    assert_eq!(
        install.directory(ConvergeDirectory::Packs),
        Err(InspectionFailure::Failed)
    );
    install.runner.failure = Some(InspectionFailure::TimedOut);
    assert_eq!(
        install.file(ConvergeFile::Flags, Path::new("/private/flags")),
        Err(InspectionFailure::TimedOut)
    );
    assert_eq!(install.runner.calls.len(), 2);
}

#[test]
fn test_a_missing_log_directory_is_created_because_the_daemon_logs_into_it() {
    let root = Scratch::new();
    let log = root.0.join("private/log");
    let target = root.0.join("target");
    let mut install = ConvergeInstaller::new(
        Script::default(),
        "/fixture/sudo".into(),
        target.clone(),
        log.clone(),
    );
    assert_eq!(install.log_directory(), Ok(true));
    assert!(log.is_dir());
    assert_eq!(install.log_directory(), Ok(false));
    assert!(!target.exists());
    assert!(install.runner.calls.is_empty());
}

#[test]
fn an_uncreatable_log_directory_is_not_reported_as_created() {
    let root = Scratch::new();
    let log = root.0.join("file");
    std::fs::write(&log, "retained").unwrap();
    let mut install = ConvergeInstaller::new(
        Script::default(),
        "/fixture/sudo".into(),
        root.0.join("target"),
        log.clone(),
    );
    assert_eq!(install.log_directory(), Err(InspectionFailure::Failed));
    assert_eq!(std::fs::read_to_string(log).unwrap(), "retained");
    assert!(install.runner.calls.is_empty());
}
