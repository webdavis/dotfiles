use super::*;
use crate::{CommandIo, CommandOutput};
use std::ffi::OsStr;
use std::path::Path;

struct Scripted {
    case: serde_json::Value,
    calls: Vec<Vec<String>>,
}
impl CommandRunner for Scripted {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(
            io,
            CommandIo::Inspection {
                merge_stderr: false
            }
        );
        self.calls.push(
            std::iter::once(program.to_str().unwrap().to_owned())
                .chain(args.iter().map(|s| s.to_str().unwrap().to_owned()))
                .collect(),
        );
        if let Some(failure) = self.case.get("failure") {
            return Err(if failure == "timeout" {
                InspectionFailure::TimedOut
            } else {
                InspectionFailure::Unavailable
            });
        }
        Ok(CommandOutput {
            bytes: self.case["input"].as_str().unwrap().as_bytes().to_vec(),
            exit: self.case["exit"].as_i64().unwrap() as i32,
        })
    }
}
fn captures() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("captures.json")).unwrap()
}
fn check(case: serde_json::Value) {
    let mut sut = PostureQuery {
        program: "/fixture/osqueryi".into(),
        runner: Scripted {
            case: case.clone(),
            calls: vec![],
        },
    };
    let result = sut.read().unwrap();
    assert_eq!(
        result.exit,
        case["fields"][0].as_str().unwrap().parse::<i32>().unwrap(),
        "{}",
        case["name"]
    );
    assert_eq!(
        result.values,
        case["fields"].as_array().unwrap()[1..]
            .iter()
            .map(|x| x.as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
            .as_slice(),
        "{}",
        case["name"]
    );
    assert_eq!(
        serde_json::to_value(sut.runner.calls).unwrap(),
        case["calls"],
        "{}",
        case["name"]
    );
    assert_eq!(
        result.reading().values,
        result.values.each_ref().map(String::as_str)
    );
}

#[test]
fn the_trio_uses_one_captured_query_and_keeps_first_row_scalar_bytes() {
    check(captures().remove(0));
}

#[test]
fn query_streams_and_legacy_scalars_keep_captured_diagnostic_values() {
    for case in captures().into_iter().skip(1).filter(|c| c["exit"] == 0) {
        check(case);
    }
}

#[test]
fn a_failed_query_keeps_its_status_and_discards_healthy_printed_values() {
    check(captures().into_iter().find(|c| c["exit"] != 0).unwrap());
    for (failure, expected) in [
        ("timeout", InspectionFailure::TimedOut),
        ("unavailable", InspectionFailure::Unavailable),
    ] {
        let mut sut = PostureQuery {
            runner: Scripted {
                case: serde_json::json!({"failure":failure}),
                calls: vec![],
            },
            program: "/fixture/osqueryi".into(),
        };
        assert_eq!(sut.read(), Err(expected));
        assert_eq!(sut.runner.calls.len(), 1);
    }
}
