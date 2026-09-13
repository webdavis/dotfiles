use super::*;
use crate::{CommandIo, CommandOutput};
use posture_application::InspectionFailure;
use posture_domain::{AgentExit, AgentState, judge_agent};
use std::{collections::VecDeque, ffi::OsStr, path::Path};
struct Script(
    VecDeque<(
        &'static str,
        Vec<String>,
        Result<CommandOutput, InspectionFailure>,
    )>,
);
impl Script {
    fn print(output: &str, exit: i32) -> Self {
        Self(VecDeque::from([(
            "/bin/launchctl",
            vec!["print".into(), "gui/501/com.webdavis.osquery-digest".into()],
            Ok(CommandOutput {
                bytes: output.as_bytes().to_vec(),
                exit,
            }),
        )]))
    }
}
impl CommandRunner for Script {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        let (wanted, argv, answer) = self.0.pop_front().expect("unexpected command");
        assert_eq!(program, Path::new(wanted));
        assert_eq!(args, argv.iter().map(OsStr::new).collect::<Vec<_>>());
        assert!(matches!(
            io,
            CommandIo::Inspection {
                merge_stderr: false
            }
        ));
        answer
    }
}
#[test]
fn loaded_agent_fields_reach_existing_crash_streak_policy() {
    let mut reader = SystemWatchdogProcesses::new(
        Script::print("\truns = 7\n\tlast exit code = -9: SIGKILL\n", 0),
        501,
    );
    let judged = judge_agent(
        Agent::Digest,
        reader.agent(Agent::Digest),
        AgentState {
            runs: Some(6),
            streak: 1,
        },
    );
    assert_eq!(judged.state.unwrap().streak, 2);
    assert!(judged.problem.unwrap().contains("last exit -9"));
    assert!(reader.runner.0.is_empty());
}
#[test]
fn sentinel_and_malformed_fields_remain_distinct_from_unloaded() {
    for (text, expected) in [
        ("last exit code = (never exited)\n", AgentExit::NeverExited),
        (
            "last exit code = prefix (never exited)\n",
            AgentExit::Unknown,
        ),
        ("runs = bad\n", AgentExit::Missing),
    ] {
        let mut reader = SystemWatchdogProcesses::new(Script::print(text, 0), 501);
        assert_eq!(
            reader.agent(Agent::Digest),
            AgentReading::Loaded {
                runs: None,
                exit: expected
            }
        );
    }
    let mut reader = SystemWatchdogProcesses::new(Script::print("last exit code = 0\n", 113), 501);
    assert_eq!(reader.agent(Agent::Digest), AgentReading::Unloaded);
}
#[test]
fn osquery_and_pns_liveness_use_read_only_commands_and_fail_closed() {
    let script = Script(VecDeque::from([
        (
            "/usr/bin/pgrep",
            vec!["-fq".into(), "/opt/osquery/.*osqueryd".into()],
            Ok(CommandOutput {
                bytes: vec![],
                exit: 0,
            }),
        ),
        (
            "/bin/launchctl",
            vec!["print".into(), "gui/501/com.webdavis.pns-daemon".into()],
            Ok(CommandOutput {
                bytes: b"state = running\npid = 42\n".to_vec(),
                exit: 0,
            }),
        ),
        (
            "/bin/kill",
            vec!["-0".into(), "42".into()],
            Ok(CommandOutput {
                bytes: vec![],
                exit: 0,
            }),
        ),
    ]));
    let mut reader = SystemWatchdogProcesses::new(script, 501);
    assert!(reader.osquery_running());
    assert_eq!(reader.pns_daemon(), DaemonHealth::Running);
    assert!(reader.runner.0.is_empty());
}
#[test]
fn a_loaded_daemon_needs_running_state_and_a_live_valid_pid() {
    for output in [
        "state = waiting\npid = 42\n",
        "state = running\npid = -1\n",
        "state = running\npid = hostile\n",
        "state = running\n",
    ] {
        let mut script = Script::print(output, 0);
        script.0[0].1[1] = "gui/501/com.webdavis.pns-daemon".into();
        let mut reader = SystemWatchdogProcesses::new(script, 501);
        assert_eq!(reader.pns_daemon(), DaemonHealth::NotRunning);
    }
}
