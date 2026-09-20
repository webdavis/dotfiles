use super::*;
use crate::test_processes::ScriptedProcesses;
use crate::{CommandIo, CommandOutput};
use posture_application::InspectionFailure;
use posture_domain::{ControlRecord, ControlValue, ControlsInput, validate_controls};
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::Path;

struct Scripted {
    responses: VecDeque<serde_json::Value>,
    calls: Vec<Vec<String>>,
}
impl CommandRunner for Scripted {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<CommandOutput, InspectionFailure> {
        let program = program.to_str().unwrap();
        assert_eq!(
            io,
            CommandIo::Inspection {
                merge_stderr: program != "/usr/bin/readlink"
            }
        );
        self.calls.push(
            std::iter::once(program.to_owned())
                .chain(args.iter().map(|s| s.to_str().unwrap().to_owned()))
                .collect(),
        );
        let next = self
            .responses
            .pop_front()
            .expect("an undeclared extra probe ran");
        if let Some(error) = next.get("failure") {
            return Err(if error == "timeout" {
                InspectionFailure::TimedOut
            } else {
                InspectionFailure::Unavailable
            });
        }
        Ok(CommandOutput {
            bytes: format!(
                "{}{}",
                next["out"].as_str().unwrap(),
                next["err"].as_str().unwrap()
            )
            .into_bytes(),
            exit: next["exit"].as_i64().unwrap() as i32,
        })
    }
}
fn control(reader: &str, target: &str) -> Control {
    let expect = match reader {
        "fdesetup_status" | "defaults_autologin" => "on",
        "csrutil_status" | "sysadminctl_guest" => "enabled",
        "pgrep_oversight" | "pgrep_lulu_extension" => "running",
        _ => "present",
    };
    validate_controls(ControlsInput::Records(&[ControlRecord {
        id: "fixture",
        tier: "verify",
        reader,
        expect,
        target,
        description: "Fixture",
        remedy: "",
    }]))
    .unwrap()
    .remove(0)
}
fn captures() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("captures.json")).unwrap()
}
/// Writes each named fixture property list, leaving out the ones declared absent.
fn lay_out(case: &serde_json::Value, sandbox: &crate::test_sandbox::Sandbox) -> [PathBuf; 3] {
    ["rules", "preferences", "login_window"].map(|role| {
        let path = sandbox.join(format!("{role}.plist"));
        if let Some(contents) = case["files"][role].as_str() {
            std::fs::write(&path, contents).expect("fixture contents");
        }
        path
    })
}
fn walks(case: &serde_json::Value) -> ScriptedProcesses {
    ScriptedProcesses::new(
        case["walk_results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|result| match result.as_array() {
                Some(pids) => Ok(pids
                    .iter()
                    .map(|pid| pid.as_i64().unwrap() as i32)
                    .collect()),
                None => Err(InspectionFailure::Failed),
            }),
    )
}
fn expected_walks(case: &serde_json::Value) -> Vec<(String, Option<u32>, Option<u32>)> {
    case["walks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|walk| {
            (
                walk[0].as_str().unwrap().to_owned(),
                Some(walk[1].as_u64().unwrap() as u32),
                None,
            )
        })
        .collect()
}
fn check(case: &serde_json::Value) {
    let controls: Vec<_> = case["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| control(x[0].as_str().unwrap(), x[1].as_str().unwrap()))
        .collect();
    let sandbox = crate::test_sandbox::Sandbox::new("control-probes");
    let [rules, preferences, login_window] = lay_out(case, &sandbox);
    let mut sut = ControlProbes {
        runner: Scripted {
            responses: case["responses"].as_array().unwrap().clone().into(),
            calls: vec![],
        },
        processes: walks(case),
        uid: case["uid"].as_u64().unwrap() as u32,
        rules,
        preferences,
        login_window,
    };
    let (values, profile) = sut.read(&controls);
    let expected = case["fields"].as_array().unwrap();
    assert_eq!(
        values,
        expected[1..]
            .iter()
            .map(|x| ControlValue::parse(x.as_str().unwrap())
                .map_or(ControlReading::Indeterminate, ControlReading::Known))
            .collect::<Vec<_>>(),
        "{}",
        case["name"]
    );
    assert_eq!(
        profile,
        match expected[0].as_str().unwrap() {
            "no_profile" => LuluProfile::Base,
            "profile_active" => LuluProfile::Active,
            _ => LuluProfile::Unconfirmed,
        },
        "{}",
        case["name"]
    );
    assert_eq!(
        serde_json::to_value(&sut.runner.calls).unwrap(),
        case["calls"],
        "{}",
        case["name"]
    );
    assert_eq!(
        sut.processes.calls,
        expected_walks(case),
        "{}",
        case["name"]
    );
    assert!(sut.runner.responses.is_empty(), "{}", case["name"]);
}

#[test]
fn all_eight_control_probes_keep_captured_arguments_output_and_profile_order() {
    check(&captures()[0]);
}

#[test]
fn failed_control_exits_and_refused_process_walks_never_believe_healthy_output() {
    check(&captures()[1]);
}

#[test]
fn lulu_reads_keep_profile_refusal_resolution_order_and_whole_string_archive_matches() {
    for case in &captures()[2..] {
        check(case);
    }
}

#[test]
fn probe_launch_and_deadline_failures_remain_indeterminate() {
    for failure in ["timeout", "unavailable"] {
        let mut sut = ControlProbes {
            runner: Scripted {
                responses: vec![serde_json::json!({"failure":failure})].into(),
                calls: vec![],
            },
            processes: ScriptedProcesses::new([]),
            uid: 501,
            rules: "/fixture/rules.plist".into(),
            preferences: "/fixture/preferences.plist".into(),
            login_window: "/fixture/loginwindow.plist".into(),
        };
        assert_eq!(
            sut.read(&[control("fdesetup_status", "")]),
            (vec![ControlReading::Indeterminate], LuluProfile::Base)
        );
        assert_eq!(
            sut.runner.calls,
            vec![vec!["/usr/bin/fdesetup".to_owned(), "status".to_owned()]]
        );
    }
}
