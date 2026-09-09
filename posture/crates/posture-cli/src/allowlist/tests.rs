use super::*;
use posture_application::{CaptureRefusal, PublicationRefusal};
fn invoke(
    words: &[&str],
    result: Result<CurationOutcome, CurationFailure>,
) -> (u8, Vec<u8>, Vec<u8>) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let args = words.iter().map(OsString::from).collect::<Vec<_>>();
    let status = execute(
        &args,
        Path::new("/deployed file"),
        |_| result,
        &mut out,
        &mut err,
    );
    (status, out, err)
}
#[test]
fn add_and_deny_pass_the_first_label_to_curation_and_ignore_extra_operands() {
    for (verb, expected) in [
        ("add", AllowlistCommand::Add("my.agent")),
        ("deny", AllowlistCommand::Deny("my.agent")),
    ] {
        let mut called = false;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let status = execute(
            &[verb.into(), "my.agent".into(), "ignored".into()],
            Path::new("/deployed"),
            |command| {
                called = true;
                assert_eq!(command, expected);
                Ok(CurationOutcome::Denied("my.agent".into()))
            },
            &mut out,
            &mut err,
        );
        assert_eq!(status, 0);
        assert!(called);
        assert_eq!(out, b"denied: my.agent\n");
        assert!(err.is_empty());
    }
}
#[test]
fn list_forwards_raw_deployed_bytes_and_calls_the_list_use_case() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        execute(
            &["list".into(), "ignored".into()],
            Path::new("/deployed"),
            |command| {
                assert_eq!(command, AllowlistCommand::List);
                Ok(CurationOutcome::Listed(vec![255, b'\n']))
            },
            &mut out,
            &mut err
        ),
        0
    );
    assert_eq!(out, [255, b'\n']);
    assert!(err.is_empty());
}
#[test]
fn malformed_subcommands_fail_usage_without_running_curation() {
    for words in [
        vec![],
        vec!["add"],
        vec!["deny"],
        vec!["unknown"],
        vec!["-a", "my.agent"],
    ] {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let args = words.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(
            execute(
                &args,
                Path::new("/deployed"),
                |_| panic!("usage performs no curation"),
                &mut out,
                &mut err
            ),
            2
        );
        assert!(out.is_empty());
        assert_eq!(err, crate::USAGE.as_bytes());
    }
}
#[test]
fn success_messages_keep_the_absolute_program_and_absent_deny_note() {
    for (result, expected) in [
        (
            CurationOutcome::Allowed {
                label: "my.agent".into(),
                program: "/home/program --arg".into(),
            },
            b"allowed: my.agent -> /home/program --arg\n".as_slice(),
        ),
        (
            CurationOutcome::NotPresent("my.agent".into()),
            b"not present: my.agent\n",
        ),
    ] {
        assert_eq!(
            invoke(&["add", "my.agent"], Ok(result)),
            (0, expected.to_vec(), vec![])
        );
    }
}
#[test]
fn validation_capture_and_lock_refusals_are_nonzero_without_success_output() {
    for (error, expected) in [
        (
            CurationFailure::Lock,
            "failed to set up the allowlist write lock (/deployed file.lock)\n",
        ),
        (
            CurationFailure::InvalidLabel("com.apple.bad".into()),
            "refused (invalid or system label): com.apple.bad\n",
        ),
        (
            CurationFailure::Capture(CaptureRefusal::NoAgent),
            "refused: my.agent has no loaded LaunchAgent to capture an identity from; load it and re-run\n",
        ),
        (
            CurationFailure::Capture(CaptureRefusal::Hash("/plist".into())),
            "refused: sha256 hash capture failed for /plist; not writing an unpinned tuple\n",
        ),
    ] {
        assert_eq!(
            invoke(&["add", "my.agent"], Err(error)),
            (1, vec![], expected.as_bytes().to_vec())
        );
    }
}
#[test]
fn apply_failure_reports_rollback_and_possible_partial_deployment_honestly() {
    let (_, out, err) = invoke(
        &["deny", "my.agent"],
        Err(CurationFailure::Publication(PublicationRefusal::Apply {
            rollback_error: None,
        })),
    );
    assert!(out.is_empty());
    let message = String::from_utf8(err).unwrap();
    assert!(message.contains("source has been rolled back"));
    assert!(message.contains("Deployment may be partial"));
    assert!(!message.contains("nothing was deployed"));
}
#[test]
fn rollback_failure_is_reported_without_claiming_the_source_was_restored() {
    let (status, out, err) = invoke(
        &["deny", "my.agent"],
        Err(CurationFailure::Publication(PublicationRefusal::Apply {
            rollback_error: Some("write refused".into()),
        })),
    );
    assert_eq!(status, 1);
    assert!(out.is_empty());
    let message = String::from_utf8(err).unwrap();
    assert!(message.contains("rollback failed: write refused"));
    assert!(!message.contains("has been rolled back"));
}
#[test]
fn manifest_failure_keeps_the_stale_warning_and_never_reports_success() {
    for error in [
        PublicationRefusal::ManifestMissing(None),
        PublicationRefusal::ManifestRefresh("/manifest".into()),
    ] {
        let (status, out, err) = invoke(
            &["add", "my.agent"],
            Err(CurationFailure::Publication(error)),
        );
        assert_eq!(status, 1);
        assert!(out.is_empty());
        let message = String::from_utf8(err).unwrap();
        assert!(message.contains("allowlist was deployed"));
        assert!(message.contains("manifest is now STALE"));
    }
}
#[test]
fn stdout_failure_cannot_be_reported_as_success() {
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        execute(
            &["list".into()],
            Path::new("/deployed"),
            |_| Ok(CurationOutcome::Listed(b"entry\n".to_vec())),
            &mut Closed,
            &mut Vec::new()
        ),
        1
    );
}

#[test]
fn invalid_label_diagnostics_keep_the_original_argument_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        execute(
            &["add".into(), OsString::from_vec(vec![255])],
            Path::new("/deployed"),
            |_| Err(CurationFailure::InvalidLabel("�".into())),
            &mut out,
            &mut err
        ),
        1
    );
    assert!(out.is_empty());
    assert_eq!(err, b"refused (invalid or system label): \xff\n");
}
