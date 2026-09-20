use crate::test_sandbox::Sandbox;
use crate::{CommandIo, CommandOutput, CommandRunner};
use posture_application::InspectionFailure;
use std::{
    ffi::{OsStr, OsString},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub(in crate::converge) struct Call {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub io: String,
}
#[derive(Default)]
pub(in crate::converge) struct Script {
    pub calls: Vec<Call>,
    pub exit: i32,
    pub bytes: Vec<u8>,
    pub failure: Option<InspectionFailure>,
}
impl CommandRunner for Script {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        self.calls.push(Call {
            program: program.into(),
            args: args.iter().map(|arg| (*arg).to_os_string()).collect(),
            io: format!("{io:?}"),
        });
        if let Some(error) = self.failure {
            return Err(error);
        }
        Ok(CommandOutput {
            bytes: self.bytes.clone(),
            exit: self.exit,
        })
    }
}
pub(in crate::converge) fn call(program: &str, args: &[&str], io: CommandIo<'_>) -> Call {
    Call {
        program: program.into(),
        args: args.iter().map(OsString::from).collect(),
        io: format!("{io:?}"),
    }
}
/// A private scratch directory, 0700 like the staging tree the converge
/// reads, removed when the test drops it.
pub(in crate::converge) fn scratch() -> Sandbox {
    let sandbox = Sandbox::new("converge-native");
    fs::set_permissions(sandbox.path(), fs::Permissions::from_mode(0o700)).unwrap();
    sandbox
}
