use super::*;
use crate::{CommandIo, CommandOutput};
use posture_application::InspectionFailure;
use std::{ffi::OsStr, path::Path};
struct Runner {
    calls: Vec<Vec<String>>,
    result: Result<CommandOutput, InspectionFailure>,
}
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(program, Path::new("/private/fixture/osascript"));
        assert_eq!(
            io,
            CommandIo::Inspection {
                merge_stderr: false
            }
        );
        self.calls.push(
            args.iter()
                .map(|arg| arg.to_str().unwrap().into())
                .collect(),
        );
        match &self.result {
            Ok(output) => Ok(CommandOutput {
                bytes: output.bytes.clone(),
                exit: output.exit,
            }),
            Err(error) => Err(*error),
        }
    }
}
fn subject(result: Result<CommandOutput, InspectionFailure>) -> LastResortBanner<Runner> {
    LastResortBanner::new(
        Runner {
            calls: vec![],
            result,
        },
        "/private/fixture/osascript".into(),
    )
}
#[test]
fn direct_independent_alarm_escapes_backslashes_before_quotes_and_uses_sosumi() {
    let mut sut = subject(Ok(CommandOutput {
        bytes: vec![],
        exit: 0,
    }));
    assert_eq!(
        sut.alarm(r#"title\"quoted"#, "line one\nC:\\path \"end\""),
        Ok(())
    );
    assert_eq!(
        sut.runner.calls,
        vec![vec![
            "-e".to_string(),
            concat!(
                r#"display notification "line one"#,
                "\n",
                r#"C:\\path \"end\"" with title "title\\\"quoted" sound name "Sosumi""#
            )
            .to_string()
        ]]
    );
}
#[test]
fn a_banner_only_reports_success_after_the_owned_command_succeeds() {
    for result in [
        Err(InspectionFailure::Unavailable),
        Err(InspectionFailure::Failed),
        Err(InspectionFailure::TimedOut),
        Ok(CommandOutput {
            bytes: b"misleading".to_vec(),
            exit: 1,
        }),
    ] {
        let mut sut = subject(result);
        assert_eq!(sut.alarm("Alarm", "Engine failed"), Err(AlarmFailed));
        assert_eq!(sut.runner.calls.len(), 1);
    }
}
