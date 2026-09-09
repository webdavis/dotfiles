use super::*;
use crate::converge::tests::fixture::*;
use std::os::unix::fs::symlink;

fn control(target: PathBuf) -> OsqueryRestart<Script> {
    OsqueryRestart::new(
        Script::default(),
        "/fixture/sudo".into(),
        "/trusted path/osqueryctl".into(),
        target,
    )
}

#[test]
fn vendor_plist_inspection_refuses_links_and_non_files_without_any_command() {
    let root = Scratch::new();
    let plist = root.0.join("io.osquery.agent.plist");
    let mut control = control(root.0.clone());
    assert_eq!(control.vendor_plist(), VendorPlist::Missing);
    let other = root.0.join("vendor");
    std::fs::write(&other, "plist").unwrap();
    symlink(&other, &plist).unwrap();
    assert_eq!(control.vendor_plist(), VendorPlist::Symlink);
    // Distinct owned fixtures keep the previous link and its referent unchanged.
    let regular = Scratch::new();
    std::fs::write(regular.0.join("io.osquery.agent.plist"), "plist").unwrap();
    control.target = regular.0;
    assert_eq!(control.vendor_plist(), VendorPlist::Regular);
    assert!(control.runner.calls.is_empty());
}

#[test]
fn every_control_call_keeps_the_one_resolved_path_and_its_output_contract() {
    let mut control = control("/fixture/target".into());
    assert_eq!(control.config_check(), Ok(()));
    assert_eq!(control.stop(), Ok(()));
    assert_eq!(control.start(), Ok(()));
    assert_eq!(
        control.runner.calls,
        [
            call(
                "/fixture/sudo",
                &["-n", "/trusted path/osqueryctl", "config-check"],
                CommandIo::Inspection {
                    merge_stderr: false
                }
            ),
            call(
                "/fixture/sudo",
                &["-n", "/trusted path/osqueryctl", "stop"],
                CommandIo::Inspection {
                    merge_stderr: false
                }
            ),
            call(
                "/fixture/sudo",
                &["-n", "/trusted path/osqueryctl", "start"],
                CommandIo::InheritAll
            )
        ]
    );
}

#[test]
fn control_exit_and_transport_failures_are_returned_to_the_restart_use_case() {
    let mut control = control("/fixture/target".into());
    control.runner.exit = 3;
    assert_eq!(control.config_check(), Err(InspectionFailure::Failed));
    control.runner.failure = Some(InspectionFailure::TimedOut);
    assert_eq!(control.stop(), Err(InspectionFailure::TimedOut));
    assert_eq!(control.start(), Err(InspectionFailure::TimedOut));
    assert_eq!(control.runner.calls.len(), 3);
}
