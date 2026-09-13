use crate::{CommandIo, CommandRunner};
use posture_application::{InspectionFailure, SshCommandResult, SshCompleted};
use std::{ffi::OsStr, path::Path};
mod files;
mod keyscan;
mod launchd;
mod sshd;
pub use files::SshFileInstaller;
pub use keyscan::SshKeyscan;
pub use launchd::SshLaunchd;
pub use sshd::SshdCommand;

fn run(
    runner: &mut impl CommandRunner,
    sudo: Option<&Path>,
    executable: &Path,
    args: &[&OsStr],
    io: CommandIo<'_>,
) -> SshCommandResult {
    let result = if let Some(sudo) = sudo {
        let mut privileged = vec![OsStr::new("-n"), executable.as_os_str()];
        privileged.extend_from_slice(args);
        runner.run_completed(sudo, &privileged, io)
    } else {
        runner.run_completed(executable, args, io)
    };
    result.map(|output| SshCompleted {
        status: output.exit,
        output: output.bytes,
    })
}

fn runnable(path: &Path) -> bool {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // access reads the live execute permission, including access-control entries; it never runs it.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

const CAPTURE: CommandIo<'static> = CommandIo::Inspection { merge_stderr: true };

#[cfg(test)]
mod tests;
