use super::*;
use std::cell::RefCell;

fn ok() -> LaunchctlOutput {
    LaunchctlOutput {
        success: true,
        stdout: String::new(),
        stderr: String::new(),
    }
}

fn ok_stdout(stdout: &str) -> LaunchctlOutput {
    LaunchctlOutput {
        success: true,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

fn err(stderr: &str) -> LaunchctlOutput {
    LaunchctlOutput {
        success: false,
        stdout: String::new(),
        stderr: stderr.to_string(),
    }
}

/// A runner whose answers are scripted in call order, and which records every
/// argv it was handed so a test can assert the exact command built.
struct ScriptedRunner {
    responses: RefCell<std::collections::VecDeque<LaunchctlOutput>>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl ScriptedRunner {
    fn new(responses: Vec<LaunchctlOutput>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl LaunchctlRunner for ScriptedRunner {
    fn run(&self, args: &[&str]) -> LaunchctlOutput {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|word| (*word).to_string()).collect());
        self.responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| err("no scripted response left"))
    }
}

fn uid() -> u32 {
    // SAFETY: takes no argument and cannot fail.
    unsafe { libc::getuid() }
}

fn controller(runner: ScriptedRunner, home: &str) -> LaunchdServiceController<ScriptedRunner> {
    LaunchdServiceController {
        runner,
        home: home.to_string(),
    }
}

/// A home directory with the label's plist already on disk, the way a
/// `chezmoi apply` leaves one.
fn home_with_plist(label: &str) -> String {
    let home = crate::state_fixtures::scratch("gateway-launchd");
    std::fs::create_dir_all(home.join("Library/LaunchAgents")).expect("the LaunchAgents dir");
    std::fs::write(
        home.join("Library/LaunchAgents")
            .join(format!("{label}.plist")),
        "<plist/>",
    )
    .expect("the plist");
    home.to_string_lossy().into_owned()
}

const LABEL: &str = "com.webdavis.pns-daemon";

#[test]
fn start_refuses_a_missing_plist_before_running_launchctl_at_all() {
    let home = crate::state_fixtures::scratch("gateway-launchd-missing");
    let runner = ScriptedRunner::new(vec![]);
    let sut = controller(runner, &home.to_string_lossy());
    let path = format!("{}/Library/LaunchAgents/{LABEL}.plist", home.display());
    assert_eq!(sut.start(LABEL), Err(ServiceError::MissingPlist(path)));
    assert!(sut.runner.calls().is_empty(), "launchctl must not run");
}

#[test]
fn start_bootstraps_the_plist_it_found_on_disk() {
    let home = home_with_plist(LABEL);
    let plist = format!("{home}/Library/LaunchAgents/{LABEL}.plist");
    let sut = controller(ScriptedRunner::new(vec![ok()]), &home);
    assert_eq!(sut.start(LABEL), Ok(()));
    assert_eq!(
        sut.runner.calls(),
        vec![vec![
            "bootstrap".to_string(),
            format!("gui/{}", uid()),
            plist
        ]]
    );
}

#[test]
fn start_falls_back_to_a_plain_kickstart_when_already_loaded() {
    let home = home_with_plist(LABEL);
    let sut = controller(
        ScriptedRunner::new(vec![err("Bootstrap failed: 5: Input/output error"), ok()]),
        &home,
    );
    assert_eq!(sut.start(LABEL), Ok(()));
    let calls = sut.runner.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[1],
        vec!["kickstart".to_string(), format!("gui/{}/{LABEL}", uid())]
    );
}

#[test]
fn start_surfaces_the_fallback_kickstarts_own_failure() {
    let home = home_with_plist(LABEL);
    let sut = controller(
        ScriptedRunner::new(vec![
            err("Bootstrap failed: 5: Input/output error"),
            err("  something else broke  "),
        ]),
        &home,
    );
    assert_eq!(
        sut.start(LABEL),
        Err(ServiceError::Failed("something else broke".to_string()))
    );
}

#[test]
fn start_surfaces_every_other_bootstrap_failure_trimmed() {
    let home = home_with_plist(LABEL);
    let sut = controller(ScriptedRunner::new(vec![err("  disk full  ")]), &home);
    assert_eq!(
        sut.start(LABEL),
        Err(ServiceError::Failed("disk full".to_string()))
    );
}

#[test]
fn stop_boots_out_the_service_target() {
    let sut = controller(ScriptedRunner::new(vec![ok()]), "/Users/nobody");
    assert_eq!(sut.stop(LABEL), Ok(()));
    assert_eq!(
        sut.runner.calls(),
        vec![vec![
            "bootout".to_string(),
            format!("gui/{}/{LABEL}", uid())
        ]]
    );
}

#[test]
fn stop_reads_not_loaded_as_a_named_error_rather_than_a_generic_one() {
    let sut = controller(
        ScriptedRunner::new(vec![err("Boot-out failed: 3: No such process")]),
        "/Users/nobody",
    );
    assert_eq!(sut.stop(LABEL), Err(ServiceError::NotLoaded));
}

#[test]
fn stop_surfaces_every_other_failure_trimmed() {
    let sut = controller(
        ScriptedRunner::new(vec![err("  denied  ")]),
        "/Users/nobody",
    );
    assert_eq!(
        sut.stop(LABEL),
        Err(ServiceError::Failed("denied".to_string()))
    );
}

#[test]
fn restart_kickstarts_with_the_kill_flag() {
    let sut = controller(ScriptedRunner::new(vec![ok()]), "/Users/nobody");
    assert_eq!(sut.restart(LABEL), Ok(()));
    assert_eq!(
        sut.runner.calls(),
        vec![vec![
            "kickstart".to_string(),
            "-k".to_string(),
            format!("gui/{}/{LABEL}", uid())
        ]]
    );
}

#[test]
fn restart_reads_not_loaded_as_a_named_error_rather_than_a_generic_one() {
    let sut = controller(
        ScriptedRunner::new(vec![err(
            "Could not find service \"com.webdavis.pns-daemon\" in domain for user gui: 501",
        )]),
        "/Users/nobody",
    );
    assert_eq!(sut.restart(LABEL), Err(ServiceError::NotLoaded));
}

#[test]
fn restart_surfaces_every_other_failure_trimmed() {
    let sut = controller(
        ScriptedRunner::new(vec![err("  wedged  ")]),
        "/Users/nobody",
    );
    assert_eq!(
        sut.restart(LABEL),
        Err(ServiceError::Failed("wedged".to_string()))
    );
}

#[test]
fn status_prints_the_service_target() {
    let sut = controller(
        ScriptedRunner::new(vec![ok_stdout("state = not running\n")]),
        "/Users/nobody",
    );
    assert_eq!(sut.status(LABEL), Ok(ServiceState::Loaded));
    assert_eq!(
        sut.runner.calls(),
        vec![vec!["print".to_string(), format!("gui/{}/{LABEL}", uid())]]
    );
}

/// The real shape `launchctl print` answers with: a top-level `state` and
/// `pid`, then nested endpoints whose OWN `state` lines must not be read as
/// the service's, and a `job state` line that must not be read as `state`
/// either.
const RUNNING_PRINT_OUTPUT: &str = "\
gui/501/com.webdavis.pns-daemon = {
\tstate = running
\tpath = /Users/stephen/Library/LaunchAgents/com.webdavis.pns-daemon.plist
\tpid = 86635
\tendpoints = {
\t\tcom.apple.xpc.launchd.io = {
\t\t\tstate = active
\t\t}
\t}
\tjob state = running
}
";

#[test]
fn status_reads_the_top_level_state_and_pid_over_every_nested_one() {
    let sut = controller(
        ScriptedRunner::new(vec![ok_stdout(RUNNING_PRINT_OUTPUT)]),
        "/Users/nobody",
    );
    assert_eq!(sut.status(LABEL), Ok(ServiceState::Running { pid: 86635 }));
}

#[test]
fn status_reads_loaded_but_not_running_with_no_pid() {
    let sut = controller(
        ScriptedRunner::new(vec![ok_stdout("state = not running\n")]),
        "/Users/nobody",
    );
    assert_eq!(sut.status(LABEL), Ok(ServiceState::Loaded));
}

#[test]
fn status_reads_a_launchctl_refusal_as_not_loaded_rather_than_a_failure() {
    let sut = controller(
        ScriptedRunner::new(vec![err(
            "Bad request.\nCould not find service \"com.webdavis.pns-daemon\" in domain for user gui: 501",
        )]),
        "/Users/nobody",
    );
    assert_eq!(sut.status(LABEL), Ok(ServiceState::NotLoaded));
}

#[test]
fn status_surfaces_every_other_failure_trimmed() {
    let sut = controller(
        ScriptedRunner::new(vec![err("  timed out  ")]),
        "/Users/nobody",
    );
    assert_eq!(
        sut.status(LABEL),
        Err(ServiceError::Failed("timed out".to_string()))
    );
}

#[test]
fn status_falls_back_to_loaded_when_no_state_line_is_readable_at_all() {
    let sut = controller(
        ScriptedRunner::new(vec![ok_stdout("nothing here\n")]),
        "/Users/nobody",
    );
    assert_eq!(sut.status(LABEL), Ok(ServiceState::Loaded));
}
