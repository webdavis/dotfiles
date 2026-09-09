use super::*;
mod fixture;
use fixture::*;

#[test]
fn test_a_converged_tree_is_a_silent_no_op_nothing_printed_nothing_privileged_no_restart() {
    let mut f = Fixture::new();
    assert_eq!(f.run(), Ok(()));
    assert!(f.events.borrow().is_empty());
    assert!(f.writes().is_empty());
    assert_eq!(f.calls.borrow().last().unwrap(), "log");
    assert!(f.dropped.get());
}

#[test]
fn every_missing_named_file_is_installed_from_the_retained_snapshot_then_restarted() {
    for file in ConvergeFile::ALL {
        let mut f = Fixture::new();
        f.live.changed = Some(file);
        assert_eq!(f.run(), Ok(()));
        assert_eq!(
            f.writes(),
            [format!(
                "write:{}:/snapshot/{}",
                file.relative_path(),
                file.relative_path()
            )]
        );
        assert_eq!(
            *f.events.borrow(),
            [
                ConvergeEvent::FileInstalled(file, Drift::Absent),
                ConvergeEvent::Restarted(restarted())
            ]
        );
        assert!(f.dropped.get());
    }
}

#[test]
fn both_directory_verdicts_are_taken_before_an_irregular_one_refuses_every_repair() {
    let mut f = Fixture::new();
    f.live.target_drift = true;
    f.live.irregular_packs = true;
    assert_eq!(
        f.run(),
        Err(ConvergeFailure::Preparation(
            ConvergeRefusal::IrregularDirectory(ConvergeDirectory::Packs)
        ))
    );
    assert_eq!(*f.calls.borrow(), ["stage", "probe:Target", "probe:Packs"]);
    assert!(f.dropped.get());
}

#[test]
fn files_are_probed_after_directory_repairs_so_changed_reachability_is_observed() {
    let mut f = Fixture::new();
    f.live.target_drift = true;
    f.live.files_require_repair = true;
    assert_eq!(f.run(), Ok(()));
    assert_eq!(
        &f.calls.borrow()[..5],
        [
            "stage",
            "probe:Target",
            "probe:Packs",
            "write:Target",
            "file:/snapshot/osquery.conf"
        ]
    );
    assert_eq!(f.writes(), ["write:Target"]);
    assert_eq!(
        *f.events.borrow(),
        [
            ConvergeEvent::DirectoryRepaired(ConvergeDirectory::Target, Drift::Mode),
            ConvergeEvent::Restarted(restarted())
        ]
    );
}

#[test]
fn staging_refusal_prevents_installs_log_creation_and_restart() {
    let mut f = Fixture::new();
    f.staging.refuse = true;
    assert!(matches!(
        f.run(),
        Err(ConvergeFailure::Preparation(ConvergeRefusal::Staging(_)))
    ));
    assert_eq!(*f.calls.borrow(), ["stage"]);
}

#[test]
fn failed_directory_install_prevents_file_probes_and_success_reports() {
    let mut f = Fixture::new();
    f.live.target_drift = true;
    f.install.reject = Some("directory");
    assert_eq!(
        f.run(),
        Err(ConvergeFailure::Directory(
            ConvergeDirectory::Target,
            InspectionFailure::Failed
        ))
    );
    assert_eq!(
        *f.calls.borrow(),
        ["stage", "probe:Target", "probe:Packs", "write:Target"]
    );
    assert!(f.events.borrow().is_empty());
}

#[test]
fn failed_file_install_prevents_later_repairs_and_restart() {
    let mut f = Fixture::new();
    f.live.changed = Some(ConvergeFile::Configuration);
    f.install.reject = Some("file");
    assert_eq!(
        f.run(),
        Err(ConvergeFailure::File(
            ConvergeFile::Configuration,
            InspectionFailure::Failed
        ))
    );
    assert_eq!(f.calls.borrow().len(), 5);
    assert!(f.events.borrow().is_empty());
}

#[test]
fn test_creating_the_log_directory_does_not_bounce_the_root_daemon() {
    let mut f = Fixture::new();
    f.install.create_log = true;
    assert_eq!(f.run(), Ok(()));
    assert_eq!(*f.events.borrow(), [ConvergeEvent::LogDirectoryCreated]);
    assert_eq!(f.calls.borrow().last().unwrap(), "log");
}

#[test]
fn failed_log_directory_creation_prevents_a_required_restart() {
    let mut f = Fixture::new();
    f.live.target_drift = true;
    f.install.reject = Some("log");
    assert_eq!(
        f.run(),
        Err(ConvergeFailure::LogDirectory(InspectionFailure::Failed))
    );
    assert_eq!(f.calls.borrow().last().unwrap(), "log");
}

#[test]
fn a_failed_report_stops_after_the_effect_and_releases_its_snapshot() {
    let mut f = Fixture::new();
    f.live.target_drift = true;
    f.reject_report = true;
    assert_eq!(f.run(), Err(ConvergeFailure::Report));
    assert_eq!(
        *f.calls.borrow(),
        ["stage", "probe:Target", "probe:Packs", "write:Target"]
    );
    assert!(f.dropped.get());
}

#[test]
fn restart_failure_retains_prior_repair_reports_without_a_restart_claim() {
    let mut f = Fixture::new();
    f.live.target_drift = true;
    f.reject_restart = true;
    assert_eq!(
        f.run(),
        Err(ConvergeFailure::Restart(RestartFailure::Deadline))
    );
    assert_eq!(
        *f.events.borrow(),
        [ConvergeEvent::DirectoryRepaired(
            ConvergeDirectory::Target,
            Drift::Mode
        )]
    );
    assert!(f.dropped.get());
}
