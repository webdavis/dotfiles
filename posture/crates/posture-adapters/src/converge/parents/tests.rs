use super::*;
use crate::test_processes::ScriptedProcesses;

#[test]
fn test_the_restart_is_judged_on_the_ppid_1_parent_never_on_an_arbitrary_worker() {
    let mut reader = OsqueryParents::new(ScriptedProcesses::new([Ok(vec![42, 99])]));
    assert_eq!(reader.daemon_parent(), Ok(ParentPid::parse("42")));
    assert_eq!(
        reader.0.calls,
        [("osqueryd".to_owned(), None, Some(1), None)],
        "the walk asks for the launchd-parented daemon by name"
    );
}

#[test]
fn only_a_completed_no_match_is_absence_while_a_refused_walk_stays_unknown() {
    let mut reader = OsqueryParents::new(ScriptedProcesses::new([
        Ok(vec![]),
        Err(InspectionFailure::Failed),
        Err(InspectionFailure::TimedOut),
    ]));
    assert_eq!(reader.daemon_parent(), Ok(None));
    assert_eq!(reader.daemon_parent(), Err(InspectionFailure::Failed));
    assert_eq!(reader.daemon_parent(), Err(InspectionFailure::TimedOut));
}

#[test]
fn a_successful_but_unusable_parent_reading_cannot_establish_fresh_host_absence() {
    let mut reader = OsqueryParents::new(ScriptedProcesses::new([Ok(vec![0])]));
    assert_eq!(reader.daemon_parent(), Err(InspectionFailure::Failed));
    assert_eq!(reader.0.calls.len(), 1);
}
