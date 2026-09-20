use super::*;
use std::cell::RefCell;

const LABEL: &str = "com.webdavis.pns-daemon";

/// A `ServiceController` whose four answers are set before the call and
/// whose calls are recorded, so a test can assert both what `run_gateway`
/// decided to print and exactly which port method it reached for.
#[derive(Default)]
struct ScriptedController {
    start_result: RefCell<Option<Result<(), ServiceError>>>,
    stop_result: RefCell<Option<Result<(), ServiceError>>>,
    restart_result: RefCell<Option<Result<(), ServiceError>>>,
    status_result: RefCell<Option<Result<ServiceState, ServiceError>>>,
    calls: RefCell<Vec<String>>,
}

impl ServiceController for ScriptedController {
    fn start(&self, label: &str) -> Result<(), ServiceError> {
        self.calls.borrow_mut().push(format!("start {label}"));
        self.start_result
            .borrow_mut()
            .take()
            .expect("start was not scripted")
    }
    fn stop(&self, label: &str) -> Result<(), ServiceError> {
        self.calls.borrow_mut().push(format!("stop {label}"));
        self.stop_result
            .borrow_mut()
            .take()
            .expect("stop was not scripted")
    }
    fn restart(&self, label: &str) -> Result<(), ServiceError> {
        self.calls.borrow_mut().push(format!("restart {label}"));
        self.restart_result
            .borrow_mut()
            .take()
            .expect("restart was not scripted")
    }
    fn status(&self, label: &str) -> Result<ServiceState, ServiceError> {
        self.calls.borrow_mut().push(format!("status {label}"));
        self.status_result
            .borrow_mut()
            .take()
            .expect("status was not scripted")
    }
}

#[test]
fn a_bare_or_unknown_verb_prints_usage_and_refuses() {
    let controller = ScriptedController::default();
    for verb in ["", "bogus"] {
        assert_eq!(run_gateway(verb, "macos", Some(LABEL), &controller), 2);
    }
    assert!(controller.calls.borrow().is_empty(), "launchctl never runs");
}

#[test]
fn off_macos_every_known_verb_refuses_before_touching_the_label_or_the_controller() {
    let controller = ScriptedController::default();
    for verb in ["start", "stop", "restart", "status"] {
        assert_eq!(run_gateway(verb, "linux", None, &controller), 2);
    }
    assert!(controller.calls.borrow().is_empty());
}

#[test]
fn a_missing_service_key_refuses_every_verb_by_name() {
    let controller = ScriptedController::default();
    for verb in ["start", "stop", "restart", "status"] {
        assert_eq!(run_gateway(verb, "macos", None, &controller), 2);
    }
    assert!(controller.calls.borrow().is_empty());
}

#[test]
fn start_reports_the_controllers_success() {
    let controller = ScriptedController::default();
    *controller.start_result.borrow_mut() = Some(Ok(()));
    assert_eq!(run_gateway("start", "macos", Some(LABEL), &controller), 0);
    assert_eq!(*controller.calls.borrow(), vec![format!("start {LABEL}")]);
}

#[test]
fn start_reports_a_missing_plist_at_exit_two() {
    let controller = ScriptedController::default();
    *controller.start_result.borrow_mut() = Some(Err(ServiceError::MissingPlist(
        "/home/x/y.plist".to_string(),
    )));
    assert_eq!(run_gateway("start", "macos", Some(LABEL), &controller), 2);
}

#[test]
fn start_reports_every_other_failure_at_exit_one() {
    let controller = ScriptedController::default();
    *controller.start_result.borrow_mut() = Some(Err(ServiceError::Failed("boom".to_string())));
    assert_eq!(run_gateway("start", "macos", Some(LABEL), &controller), 1);
}

#[test]
fn stop_reports_the_controllers_success() {
    let controller = ScriptedController::default();
    *controller.stop_result.borrow_mut() = Some(Ok(()));
    assert_eq!(run_gateway("stop", "macos", Some(LABEL), &controller), 0);
}

#[test]
fn stop_reads_not_loaded_as_success_rather_than_a_refusal() {
    let controller = ScriptedController::default();
    *controller.stop_result.borrow_mut() = Some(Err(ServiceError::NotLoaded));
    assert_eq!(run_gateway("stop", "macos", Some(LABEL), &controller), 0);
    assert_eq!(*controller.calls.borrow(), vec![format!("stop {LABEL}")]);
}

#[test]
fn stop_reports_every_other_failure_at_exit_one() {
    let controller = ScriptedController::default();
    *controller.stop_result.borrow_mut() = Some(Err(ServiceError::Failed("boom".to_string())));
    assert_eq!(run_gateway("stop", "macos", Some(LABEL), &controller), 1);
}

#[test]
fn restart_reports_the_controllers_success() {
    let controller = ScriptedController::default();
    *controller.restart_result.borrow_mut() = Some(Ok(()));
    assert_eq!(run_gateway("restart", "macos", Some(LABEL), &controller), 0);
    assert_eq!(*controller.calls.borrow(), vec![format!("restart {LABEL}")]);
}

#[test]
fn restart_falls_back_to_start_when_the_service_was_not_loaded() {
    let controller = ScriptedController::default();
    *controller.restart_result.borrow_mut() = Some(Err(ServiceError::NotLoaded));
    *controller.start_result.borrow_mut() = Some(Ok(()));
    assert_eq!(run_gateway("restart", "macos", Some(LABEL), &controller), 0);
    assert_eq!(
        *controller.calls.borrow(),
        vec![format!("restart {LABEL}"), format!("start {LABEL}")]
    );
}

#[test]
fn restart_reports_the_fallback_starts_own_failure() {
    let controller = ScriptedController::default();
    *controller.restart_result.borrow_mut() = Some(Err(ServiceError::NotLoaded));
    *controller.start_result.borrow_mut() = Some(Err(ServiceError::MissingPlist(
        "/home/x/y.plist".to_string(),
    )));
    assert_eq!(run_gateway("restart", "macos", Some(LABEL), &controller), 2);
}

#[test]
fn restart_reports_every_other_failure_at_exit_one() {
    let controller = ScriptedController::default();
    *controller.restart_result.borrow_mut() = Some(Err(ServiceError::Failed("boom".to_string())));
    assert_eq!(run_gateway("restart", "macos", Some(LABEL), &controller), 1);
    // NO FALLBACK, unlike `NotLoaded`: this is a real launchctl failure, not
    // the state the fallback exists to recover from.
    assert_eq!(*controller.calls.borrow(), vec![format!("restart {LABEL}")]);
}

#[test]
fn status_reports_running_at_exit_zero() {
    let controller = ScriptedController::default();
    *controller.status_result.borrow_mut() = Some(Ok(ServiceState::Running { pid: 42 }));
    assert_eq!(run_gateway("status", "macos", Some(LABEL), &controller), 0);
}

#[test]
fn status_reports_loaded_at_exit_zero() {
    let controller = ScriptedController::default();
    *controller.status_result.borrow_mut() = Some(Ok(ServiceState::Loaded));
    assert_eq!(run_gateway("status", "macos", Some(LABEL), &controller), 0);
}

#[test]
fn status_reports_not_loaded_at_exit_one_so_a_script_can_test_it() {
    let controller = ScriptedController::default();
    *controller.status_result.borrow_mut() = Some(Ok(ServiceState::NotLoaded));
    assert_eq!(run_gateway("status", "macos", Some(LABEL), &controller), 1);
}

#[test]
fn status_reports_every_other_failure_at_exit_one() {
    let controller = ScriptedController::default();
    *controller.status_result.borrow_mut() = Some(Err(ServiceError::Failed("boom".to_string())));
    assert_eq!(run_gateway("status", "macos", Some(LABEL), &controller), 1);
}
