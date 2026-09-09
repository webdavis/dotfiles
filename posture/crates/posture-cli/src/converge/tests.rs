use super::*;
use posture_application::{
    ConvergeEvent, ConvergeRefusal, InspectionFailure, RestartFailure, Restarted, StagingRefusal,
};
use posture_domain::{ConvergeDirectory, ConvergeFile, Drift, ParentPid};
use std::{cell::Cell, io, path::PathBuf, time::Duration};

fn config() -> Configuration {
    Configuration::read(|name| match name {
        "HOME" => Some("/fixture/home".into()),
        "PATH" => Some("/fixture/absent".into()),
        _ => None,
    })
    .unwrap()
}
fn report(
    event: ConvergeEvent,
    config: &Configuration,
    out: &mut impl Write,
) -> Result<(), Failure> {
    reporting::event(event, config, out).map_err(|_| Failure::Converge(ConvergeFailure::Report))
}

#[test]
fn test_an_unknown_argument_is_a_usage_error_never_a_silent_full_converge() {
    let called = Cell::new(false);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(
        execute(
            &["--repair".into()],
            Ok(config()),
            |_, _| {
                called.set(true);
                Ok(())
            },
            &mut stdout,
            &mut stderr
        ),
        2
    );
    assert!(!called.get());
    assert!(stdout.is_empty());
    assert_eq!(
        stderr,
        b"usage: posture converge\nosquery-converge: unknown argument: --repair\n"
    );
}

#[test]
fn test_seam_refusals_precede_argument_parsing_and_native_effects() {
    let errors = vec![
        ConfigurationRefusal::UnexpectedOverride("OSQUERY_CONVERGE_TARGET_DIR"),
        ConfigurationRefusal::UnexpectedOverride("OSQUERY_CONVERGE_SUDO"),
    ];
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        execute(
            &["bad".into()],
            Err(errors),
            |_, _| panic!("refused seam ran"),
            &mut out,
            &mut err
        ),
        2
    );
    assert!(out.is_empty());
    let text = String::from_utf8(err).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("OSQUERY_CONVERGE_TARGET_DIR is a TEST-ONLY seam"));
    assert!(text.contains("OSQUERY_CONVERGE_SUDO is a TEST-ONLY seam"));
    assert!(!text.contains("usage:"));
}

#[test]
fn unavailable_osqueryctl_returns_before_any_staging_or_target_creation() {
    let root = std::env::temp_dir().join(format!("converge-cli-absent-{}", std::process::id()));
    assert!(!root.exists());
    let mut configuration = config();
    configuration.desired = root.join("missing-desired");
    configuration.target = root.join("target");
    configuration.log_directory = root.join("log");
    configuration.osqueryctl = Some(root.join("missing-ctl"));
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        execute(&[], Ok(configuration), native::run, &mut out, &mut err),
        0
    );
    assert!(out.is_empty());
    assert!(err.is_empty());
    assert!(!root.exists());
}

#[test]
fn test_a_repair_says_which_file_it_repaired_and_why() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        execute(
            &[],
            Ok(config()),
            |config, out| {
                report(
                    ConvergeEvent::DirectoryRepaired(ConvergeDirectory::Packs, Drift::Mode),
                    config,
                    out,
                )?;
                report(
                    ConvergeEvent::FileInstalled(ConvergeFile::Flags, Drift::Absent),
                    config,
                    out,
                )?;
                report(
                    ConvergeEvent::Restarted(Restarted {
                        parent: ParentPid::parse("42").unwrap(),
                        settled_for: Duration::from_secs(5),
                    }),
                    config,
                    out,
                )
            },
            &mut out,
            &mut err
        ),
        0
    );
    assert_eq!(out,b"osquery-converge: repaired /var/osquery/packs (mode drift)\nosquery-converge: installed /var/osquery/osquery.flags (missing)\nosquery-converge: restarted osqueryd (parent pid 42, still up after 5s)\n");
    assert!(err.is_empty());
}

#[test]
fn test_a_start_that_fails_after_a_successful_stop_leaves_no_success_line() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        execute(
            &[],
            Ok(config()),
            |config, out| {
                report(
                    ConvergeEvent::FileInstalled(ConvergeFile::Configuration, Drift::Content),
                    config,
                    out,
                )?;
                Err(Failure::Converge(ConvergeFailure::Restart(
                    RestartFailure::Start(InspectionFailure::Failed),
                )))
            },
            &mut out,
            &mut err
        ),
        1
    );
    assert_eq!(
        out,
        b"osquery-converge: installed /var/osquery/osquery.conf (content drift)\n"
    );
    assert!(
        String::from_utf8(err)
            .unwrap()
            .contains("'osqueryctl start' FAILED")
    );
}

#[test]
fn every_retained_staging_refusal_reaches_stderr_without_a_success_claim() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let errors = vec![
        StagingRefusal::MissingFile(PathBuf::from("/desired/flags")),
        StagingRefusal::SymlinkEntry(PathBuf::from("/desired/planted")),
    ];
    assert_eq!(
        execute(
            &[],
            Ok(config()),
            |_, _| Err(Failure::Converge(ConvergeFailure::Preparation(
                ConvergeRefusal::Staging(errors)
            ))),
            &mut out,
            &mut err
        ),
        1
    );
    assert!(out.is_empty());
    let text = String::from_utf8(err).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("/desired/flags"));
    assert!(text.contains("/desired/planted"));
    assert!(text.contains("symlink"));
}

#[test]
fn a_closed_report_stream_makes_the_command_fail() {
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut err = Vec::new();
    assert_eq!(
        execute(
            &[],
            Ok(config()),
            |config, out| report(ConvergeEvent::LogDirectoryCreated, config, out),
            &mut Closed,
            &mut err
        ),
        1
    );
    assert!(String::from_utf8(err).unwrap().contains("report"));
}
