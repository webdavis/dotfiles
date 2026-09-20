use super::*;
use crate::test_sandbox::Sandbox;
use posture_adapters::{CommandIo, CommandOutput};
use posture_application::{ClockUnavailable, InspectionFailure, WallTime};
use posture_domain::{AgentLabels, AuditBounds, ManifestAuthority};
use std::{
    cell::RefCell,
    ffi::OsStr,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    rc::Rc,
    time::Duration,
};
#[derive(Default)]
struct Effects {
    calls: Vec<&'static str>,
    requests: Vec<String>,
    unhealthy: bool,
    alarm_failed: bool,
    pns_failed: bool,
}
#[derive(Clone, Default)]
struct Runner(Rc<RefCell<Effects>>);
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        let mut effects = self.0.borrow_mut();
        match program.to_str().unwrap() {
            "/bin/launchctl" => {
                assert_eq!(args[0], "print");
                // The live pid is this process, so the daemon reading is
                // taken from a process that genuinely exists.
                let body = if args[1]
                    .to_string_lossy()
                    .ends_with("com.webdavis.pns-daemon")
                {
                    if effects.unhealthy {
                        "state = waiting\n".to_owned()
                    } else {
                        format!("state = running\npid = {}\n", std::process::id())
                    }
                } else {
                    "runs = 1\nlast exit code = (never exited)\n".to_owned()
                };
                Ok(CommandOutput {
                    bytes: body.into_bytes(),
                    exit: 0,
                })
            }
            "/fixture/osascript" => {
                effects.calls.push("alarm");
                assert_eq!(args[0], "-e");
                assert!(args[1].to_string_lossy().contains("sound name \"Sosumi\""));
                if effects.alarm_failed {
                    Err(InspectionFailure::Failed)
                } else {
                    Ok(CommandOutput {
                        bytes: vec![],
                        exit: 0,
                    })
                }
            }
            _ if program.file_name() == Some(OsStr::new("pns")) => {
                assert_eq!(args, [OsStr::new("send"), OsStr::new("--json")]);
                let CommandIo::Input(input) = io else {
                    panic!("stdin request required")
                };
                let text = String::from_utf8(input.to_vec()).unwrap();
                let identity = text
                    .split("\"request_id\":\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .to_owned();
                effects.calls.push("submit");
                effects.requests.push(text);
                if effects.pns_failed {
                    return Err(InspectionFailure::TimedOut);
                }
                Ok(CommandOutput { bytes:format!(r#"{{"schema":"pns.result/1","request_id":"{identity}","status":"delivered","diagnostics":["ledger_committed"]}}"#).into_bytes(),exit:0 })
            }
            _ => panic!("unexpected command {program:?}"),
        }
    }
}
/// A scripted process table, so the assembled reads never walk the live
/// machine: every walk answers with one process id when the daemon is
/// declared present, and none when it is not.
struct Processes(bool);
impl ProcessLookup for Processes {
    fn matching(
        &mut self,
        _name: &str,
        _uid: Option<u32>,
        _parent: Option<u32>,
        _directory: Option<&Path>,
    ) -> Result<Vec<i32>, InspectionFailure> {
        Ok(if self.0 { vec![1] } else { vec![] })
    }
}
struct Time;
impl Clock for Time {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        Ok(WallTime {
            seconds: 10000,
            utc_day: "2026-09-13".into(),
        })
    }
}
struct Gateway;
impl GatewayHealth for Gateway {
    fn status(&mut self) -> Option<u16> {
        Some(405)
    }
}
/// The watchdog's configuration over a directory that removes itself. Hold
/// the sandbox for as long as the configuration is used.
fn configuration() -> (Sandbox, Configuration) {
    let sandbox = Sandbox::new("watchdog-command");
    // Canonical, because the audit reports the paths it resolved and a
    // symlinked temporary directory would not match them.
    let dir = fs::canonicalize(sandbox.path()).unwrap();
    let pns = dir.join("pns");
    fs::write(&pns, b"authorized").unwrap();
    fs::set_permissions(&pns, fs::Permissions::from_mode(0o755)).unwrap();
    let pipeline = dir.join("pipeline");
    let managed_bin = dir.join("bin");
    let tuple = format!(
        "04dd9c7e9464019a848b69db2ed9a9b2a7def45b169e44627e7e613d67ff18ce 0755 {} {}\n",
        fs::metadata(&pns).unwrap().uid(),
        pns.display()
    );
    fs::write(&pipeline, &tuple).unwrap();
    fs::write(&managed_bin, &tuple).unwrap();
    let snapshots = dir.join("snapshots");
    fs::write(
        &snapshots,
        b"{\"name\":\"heartbeat_canary\",\"unixTime\":9999}\n",
    )
    .unwrap();
    let pns_ledger = dir.join("pns.db");
    let db = rusqlite::Connection::open(&pns_ledger).unwrap();
    db.execute_batch("PRAGMA user_version=8; CREATE TABLE ledger_legs(acknowledged INTEGER,deadlettered_at BLOB);").unwrap();
    db.execute_batch("CREATE TABLE delivery_health(id INTEGER PRIMARY KEY CHECK(id=1), previous_pending INTEGER,
        growth INTEGER NOT NULL DEFAULT 0 CHECK(growth BETWEEN 0 AND 2),
        generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
        acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged >= 0 AND acknowledged <= generation));
        INSERT INTO delivery_health(id) VALUES(1);").unwrap();
    fs::set_permissions(&pns_ledger, fs::Permissions::from_mode(0o600)).unwrap();
    let configuration = Configuration {
        snapshots,
        state: dir.join("state"),
        legacy_queue: dir.join("legacy.db"),
        pns_ledger,
        pipeline,
        managed_bin,
        authority: [ManifestAuthority::ExplicitOverride; 2],
        notify: crate::command_notify(&pns),
        agents: AgentLabels::default(),
        pns,
        alarm: "/fixture/osascript".into(),
        gateway: "http://fixture/priority".into(),
        route_timeout: Duration::from_millis(20),
        maximum_age: 1800,
        bounds: AuditBounds::from_values("500", "8388608", "60"),
    };
    (sandbox, configuration)
}
fn call(c: Configuration, runner: &Runner) -> (u8, Vec<u8>) {
    let mut stderr = vec![];
    let status = execute(
        c,
        Time,
        Runners {
            processes: runner.clone(),
            producer: runner.clone(),
            fallback: runner.clone(),
            independent: runner.clone(),
        },
        Processes(true),
        &mut Gateway,
        &mut stderr,
    );
    (status, stderr)
}
#[test]
fn assembled_healthy_watchdog_persists_silently_without_any_delivery() {
    let (_sandbox, c) = configuration();
    let state = c.state.clone();
    let r = Runner::default();
    let (status, stderr) = call(c, &r);
    assert_eq!(status, 0);
    assert!(stderr.is_empty());
    assert!(state.is_file());
    assert!(r.0.borrow().calls.is_empty());
}
#[test]
fn assembled_pns_outage_alarms_before_a_security_submission_even_when_accepted() {
    let (_sandbox, c) = configuration();
    let state = c.state.clone();
    let r = Runner::default();
    r.0.borrow_mut().unhealthy = true;
    let (status, stderr) = call(c, &r);
    assert_eq!(status, 0);
    assert!(stderr.is_empty());
    assert!(state.is_file());
    let effects = r.0.borrow();
    assert_eq!(effects.calls, ["alarm", "submit"]);
    for field in [
        "\"delivery_class\":\"security\"",
        "\"route\":\"posture-pages\"",
    ] {
        assert!(effects.requests[0].contains(field));
    }
}
#[test]
fn assembled_alarm_failure_retries_without_advancing_state() {
    let (_sandbox, c) = configuration();
    let state = c.state.clone();
    fs::write(&state, b"{}\n").unwrap();
    let r = Runner::default();
    {
        let mut e = r.0.borrow_mut();
        e.unhealthy = true;
        e.alarm_failed = true;
    }
    let (status, stderr) = call(c, &r);
    assert_eq!(status, 1);
    assert!(!stderr.is_empty());
    assert_eq!(fs::read(state).unwrap(), b"{}\n");
    assert_eq!(r.0.borrow().calls, ["alarm", "submit"]);
}
#[test]
fn assembled_missing_pns_uses_the_existing_independent_fallback() {
    let (_sandbox, c) = configuration();
    let state = c.state.clone();
    let r = Runner::default();
    {
        let mut e = r.0.borrow_mut();
        e.unhealthy = true;
        e.pns_failed = true;
    }
    let (status, _) = call(c, &r);
    assert_eq!(status, 1);
    assert!(!state.exists());
    assert_eq!(r.0.borrow().calls, ["alarm", "submit", "alarm"]);
}

#[test]
fn ledger_health_refusals_alarm_before_accepted_submission_and_retain_failed_alarm_state() {
    for damage in [
        "DROP TABLE delivery_health",
        "DELETE FROM delivery_health",
        "UPDATE delivery_health SET generation=8, acknowledged=7",
    ] {
        let (_sandbox, c) = configuration();
        let db = rusqlite::Connection::open(&c.pns_ledger).unwrap();
        db.execute_batch(damage).unwrap();
        let before = fs::read(&c.pns_ledger).unwrap();
        let state = c.state.clone();
        fs::write(&state, b"{}\n").unwrap();
        let r = Runner::default();
        r.0.borrow_mut().alarm_failed = true;
        let (status, stderr) = call(c, &r);
        assert_eq!(status, 1, "{damage}");
        assert!(!stderr.is_empty());
        assert_eq!(fs::read(state).unwrap(), b"{}\n");
        let effects = r.0.borrow();
        assert_eq!(effects.calls.last(), Some(&"submit"));
        assert!(
            effects.calls[..effects.calls.len() - 1]
                .iter()
                .all(|call| *call == "alarm")
        );
        assert!(effects.calls.len() >= 2);
        assert_eq!(fs::read(db.path().unwrap()).unwrap(), before);
    }
}
