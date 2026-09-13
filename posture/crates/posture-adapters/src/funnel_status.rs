use crate::{CommandIo, CommandRunner};
use posture_application::InspectionFailure;
use posture_domain::{FunnelReadFailure, FunnelReading};
use std::{ffi::OsStr, path::Path};
mod projection;

pub fn read_funnel(
    runner: &mut impl CommandRunner,
    executable: &Path,
) -> Result<FunnelReading, FunnelReadFailure> {
    use std::os::unix::fs::PermissionsExt;
    if !std::fs::metadata(executable).is_ok_and(|m| m.permissions().mode() & 0o111 != 0) {
        return Err(FunnelReadFailure::MissingBinary(
            executable.to_string_lossy().into_owned(),
        ));
    }
    let output = runner
        .run_completed(
            executable,
            &[
                OsStr::new("funnel"),
                OsStr::new("status"),
                OsStr::new("--json"),
            ],
            CommandIo::Inspection {
                merge_stderr: false,
            },
        )
        .map_err(|failure| {
            FunnelReadFailure::Status(match failure {
                InspectionFailure::TimedOut => 124,
                InspectionFailure::Unavailable => 126,
                InspectionFailure::Failed => 1,
            })
        })?;
    if output.exit != 0 {
        return Err(FunnelReadFailure::Status(output.exit));
    }
    projection::read(&output.bytes)
}
