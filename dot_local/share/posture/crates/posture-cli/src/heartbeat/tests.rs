use super::*;
use posture_adapters::{CommandIo, CommandOutput};
use posture_application::{ClockUnavailable, InspectionFailure, WallTime};
use posture_domain::HeartbeatWindow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Default)]
struct Effects {
    requests: Vec<String>,
    alarms: Vec<Vec<OsString>>,
}
#[derive(Clone, Copy)]
enum Reply {
    Committed,
    Refused,
    Degraded,
    TimedOut,
    Malformed,
}
struct Runner {
    expected: PathBuf,
    reply: Option<Reply>,
    effects: Rc<RefCell<Effects>>,
}
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(program, self.expected);
        let Some(reply) = self.reply else {
            assert!(matches!(
                io,
                CommandIo::Inspection {
                    merge_stderr: false
                }
            ));
            self.effects
                .borrow_mut()
                .alarms
                .push(args.iter().map(|x| x.to_os_string()).collect());
            return Err(InspectionFailure::Failed);
        };
        assert_eq!(args, [OsStr::new("submit"), OsStr::new("--json")]);
        let CommandIo::Input(input) = io else {
            panic!("request must be stdin")
        };
        let request = String::from_utf8(input.to_vec()).unwrap();
        // The codec owns canonical JSON. This fixture extracts only its generated ASCII identity.
        let identity = request
            .split("\"request_id\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned();
        self.effects.borrow_mut().requests.push(request);
        let (status, diagnostics, exit) = match reply {
            Reply::Committed => ("accepted", "\"ledger_committed\"", 0),
            Reply::Refused => ("rejected", "", 2),
            Reply::Degraded => ("degraded", "\"ledger_unavailable\"", 0),
            Reply::TimedOut => return Err(InspectionFailure::TimedOut),
            Reply::Malformed => {
                return Ok(CommandOutput {
                    bytes: b"not JSON".to_vec(),
                    exit: 0,
                });
            }
        };
        Ok(CommandOutput{bytes:format!("{{\"schema\":\"pns.result/1\",\"request_id\":\"{identity}\",\"status\":\"{status}\",\"diagnostics\":[{diagnostics}]}}\n").into_bytes(),exit})
    }
}
struct Time;
impl Clock for Time {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        Ok(WallTime {
            seconds: 10000,
            utc_day: "2026-09-08".into(),
        })
    }
}
fn subject(bound: &str) -> (Configuration, Rc<RefCell<Effects>>) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let home = std::env::temp_dir().join(format!(
        "posture-heartbeat-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&home).unwrap();
    let snapshots = home.join("selected snapshot");
    std::fs::write(
        &snapshots,
        b"{\"name\":\"heartbeat_canary\",\"unixTime\":9983}\n",
    )
    .unwrap();
    (
        Configuration {
            snapshots,
            pns: home.join("pns"),
            alarm: home.join("osascript"),
            maximum_age: HeartbeatWindow::from_override(Some(bound)),
        },
        Rc::default(),
    )
}
fn run_case(
    config: Configuration,
    effects: Rc<RefCell<Effects>>,
    reply: Reply,
    stderr: &mut Vec<u8>,
) -> u8 {
    let runner = Runner {
        expected: config.pns.clone(),
        reply: Some(reply),
        effects: effects.clone(),
    };
    let alarm = Runner {
        expected: config.alarm.clone(),
        reply: None,
        effects,
    };
    execute(config, Time, runner, alarm, stderr)
}
#[test]
fn the_command_reads_the_selected_canary_and_submits_one_unmarked_posture_observation() {
    let (config, effects) = subject("1800");
    let path = config.snapshots.clone();
    let before = std::fs::read(&path).unwrap();
    let mut stderr = vec![];
    assert_eq!(
        run_case(config, effects.clone(), Reply::Committed, &mut stderr),
        0
    );
    let effect = effects.borrow();
    assert_eq!(effect.requests.len(), 1);
    assert!(effect.alarms.is_empty());
    let request = &effect.requests[0];
    for field in [
        "\"producer\":\"posture\"",
        "\"event\":\"heartbeat\"",
        "\"kind\":\"observation\"",
        "\"occurred_at\":10000",
        "\"route\":\"posture\"",
        "canary 17s ago",
    ] {
        assert!(request.contains(field), "{field}: {request}");
    }
    assert!(!request.contains("\"class\""));
    assert!(request.ends_with('\n'));
    assert!(stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
}
#[test]
fn engine_failure_attempts_an_independent_alarm_but_never_changes_best_effort_status() {
    for (reply, alarms) in [
        (Reply::Refused, 0),
        (Reply::Degraded, 0),
        (Reply::TimedOut, 1),
        (Reply::Malformed, 1),
    ] {
        let (config, effects) = subject("1800");
        let mut stderr = vec![];
        assert_eq!(run_case(config, effects.clone(), reply, &mut stderr), 0);
        let effect = effects.borrow();
        assert_eq!(effect.requests.len(), 1);
        assert_eq!(effect.alarms.len(), alarms);
        assert!(stderr.is_empty());
        for args in &effect.alarms {
            assert_eq!(args[0], "-e");
            assert!(args[1].to_string_lossy().contains("sound name \"Sosumi\""));
        }
    }
}
#[test]
fn invalid_literal_emits_one_fixed_diagnostic_and_still_submits_the_default_observation() {
    let (config, effects) = subject("08");
    let mut stderr = vec![];
    assert_eq!(
        run_case(config, effects.clone(), Reply::Committed, &mut stderr),
        0
    );
    assert_eq!(
        stderr,
        b"posture heartbeat: invalid OSQUERY_CANARY_MAX_AGE literal; using 1800 seconds\n"
    );
    assert!(!String::from_utf8(stderr).unwrap().contains("08"));
    assert!(effects.borrow().requests[0].contains("pipeline healthy"));
}
