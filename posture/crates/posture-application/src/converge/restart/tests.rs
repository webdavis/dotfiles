use super::*;
mod fixture;
use fixture::*;

#[test]
fn test_a_missing_vendor_plist_refuses_the_restart_and_never_stops_the_daemon() {
    let mut f = Fixture::new();
    f.control.vendor = VendorPlist::Missing;
    assert_eq!(
        f.run(),
        Err(RestartFailure::VendorPlist(VendorPlist::Missing))
    );
    assert_eq!(*f.calls.borrow(), ["vendor"]);
}

#[test]
fn test_a_symlink_standing_in_for_the_vendor_plist_refuses_the_restart() {
    let mut f = Fixture::new();
    f.control.vendor = VendorPlist::Symlink;
    assert_eq!(
        f.run(),
        Err(RestartFailure::VendorPlist(VendorPlist::Symlink))
    );
    assert_eq!(*f.calls.borrow(), ["vendor"]);
}

#[test]
fn test_a_config_the_daemon_cannot_parse_refuses_the_restart_and_never_stops_the_daemon() {
    let mut f = Fixture::new();
    f.control.reject = Some("check");
    assert_eq!(
        f.run(),
        Err(RestartFailure::Configuration(InspectionFailure::Failed))
    );
    assert_eq!(*f.calls.borrow(), ["vendor", "check"]);
}

#[test]
fn test_a_stop_that_fails_with_no_daemon_running_does_not_stop_the_run() {
    let mut f = Fixture::new();
    f.processes.previous = None;
    f.control.reject = Some("stop");
    assert_eq!(f.run(), Ok(restarted()));
    assert_eq!(
        &f.calls.borrow()[..5],
        ["vendor", "check", "parent", "stop", "start"]
    );
}

#[test]
fn test_a_start_that_fails_is_fatal_even_while_a_daemon_is_still_running() {
    let mut f = Fixture::new();
    f.control.reject = Some("start");
    assert_eq!(
        f.run(),
        Err(RestartFailure::Start(InspectionFailure::Failed))
    );
    assert_eq!(
        *f.calls.borrow(),
        ["vendor", "check", "parent", "stop", "start"]
    );
    assert_eq!(f.clock.elapsed(), Duration::ZERO);
}

#[test]
fn test_a_daemon_whose_parent_pid_never_changed_is_not_a_restart_however_the_stop_exited() {
    let mut f = Fixture::new();
    f.processes.readings = vec![(0, Some(pid(10)))];
    assert_eq!(f.run(), Err(RestartFailure::UnchangedParent(pid(10))));
    assert_eq!(f.clock.elapsed(), Duration::from_secs(1));
}

#[test]
fn test_a_restart_is_claimed_only_when_the_parent_pid_actually_changed() {
    let mut f = Fixture::new();
    f.processes.readings = vec![(0, Some(pid(10))), (500, Some(pid(20)))];
    assert_eq!(f.run(), Ok(restarted()));
    assert_eq!(f.clock.elapsed(), Duration::from_millis(1500));
    assert_eq!(
        f.processes.observed,
        [0, 0, 250, 500, 750, 1000, 1250, 1500]
    );
}

#[test]
fn test_a_daemon_that_never_comes_back_is_a_loud_failure_not_a_reported_success() {
    let mut f = Fixture::new();
    f.processes.readings = vec![(0, None)];
    assert_eq!(f.run(), Err(RestartFailure::Deadline));
    assert_eq!(f.clock.elapsed(), Duration::from_secs(1));
}

#[test]
fn test_a_daemon_that_dies_inside_the_settle_window_is_a_loud_failure() {
    let mut f = Fixture::new();
    f.processes.readings = vec![(0, Some(pid(20))), (500, None), (750, Some(pid(20)))];
    assert_eq!(f.run(), Err(RestartFailure::UnstableParent(pid(20))));
    assert_eq!(f.clock.elapsed(), Duration::from_millis(500));
}

#[test]
fn test_a_daemon_that_comes_back_under_a_new_pid_inside_the_settle_window_is_a_failure() {
    let mut f = Fixture::new();
    f.processes.readings = vec![(0, Some(pid(20))), (750, Some(pid(30)))];
    assert_eq!(f.run(), Err(RestartFailure::UnstableParent(pid(20))));
    assert_eq!(f.clock.elapsed(), Duration::from_millis(750));
}

#[test]
fn a_new_parent_at_the_deadline_is_accepted_and_observed_through_the_full_settle_window() {
    let mut f = Fixture::new();
    f.processes.readings = vec![(0, None), (1000, Some(pid(20)))];
    assert_eq!(f.run(), Ok(restarted()));
    assert_eq!(f.clock.elapsed(), Duration::from_secs(2));
}

#[test]
fn an_unknown_previous_parent_refuses_before_stopping_the_daemon() {
    let mut f = Fixture::new();
    f.processes.failure = Some(0);
    assert_eq!(
        f.run(),
        Err(RestartFailure::ParentProbe(InspectionFailure::TimedOut))
    );
    assert_eq!(*f.calls.borrow(), ["vendor", "check", "parent"]);
}
