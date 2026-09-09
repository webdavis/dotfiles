use super::*;
use crate::converge::tests::fixture::*;

#[test]
fn test_the_restart_is_judged_on_the_ppid_1_parent_never_on_an_arbitrary_worker() {
    let mut reader = OsqueryParents::new(Script {
        bytes: b"42\n99\n".to_vec(),
        ..Default::default()
    });
    assert_eq!(reader.daemon_parent(), Ok(ParentPid::parse("42")));
    assert_eq!(
        reader.0.calls,
        [call(
            "/usr/bin/pgrep",
            &["-P", "1", "-x", "osqueryd"],
            CommandIo::Inspection {
                merge_stderr: false
            }
        )]
    );
}

#[test]
fn only_a_completed_no_match_is_absence_while_failed_inspection_stays_unknown() {
    let mut reader = OsqueryParents::new(Script {
        exit: 1,
        ..Default::default()
    });
    assert_eq!(reader.daemon_parent(), Ok(None));
    reader.0.exit = 2;
    assert_eq!(reader.daemon_parent(), Err(InspectionFailure::Failed));
    reader.0.failure = Some(InspectionFailure::TimedOut);
    assert_eq!(reader.daemon_parent(), Err(InspectionFailure::TimedOut));
}

#[test]
fn a_successful_but_malformed_parent_reading_cannot_establish_fresh_host_absence() {
    for bytes in [
        b"".as_slice(),
        b"0\n",
        b"01\n",
        b" 1\n",
        b"1\r\n",
        b"10000000000\n",
        b"\xff\n",
    ] {
        let mut reader = OsqueryParents::new(Script {
            bytes: bytes.to_vec(),
            ..Default::default()
        });
        assert_eq!(reader.daemon_parent(), Err(InspectionFailure::Failed));
        assert_eq!(reader.0.calls.len(), 1);
    }
}
