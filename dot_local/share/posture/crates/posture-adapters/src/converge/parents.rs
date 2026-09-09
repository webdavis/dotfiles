use crate::{CommandIo, CommandRunner};
use posture_application::{InspectionFailure, ProcessTable};
use posture_domain::ParentPid;

pub struct OsqueryParents<R>(pub(super) R);
impl<R: CommandRunner> OsqueryParents<R> {
    pub fn new(runner: R) -> Self {
        Self(runner)
    }
}
impl<R: CommandRunner> ProcessTable for OsqueryParents<R> {
    fn daemon_parent(&mut self) -> Result<Option<ParentPid>, InspectionFailure> {
        let output = self.0.run_completed(
            std::path::Path::new("/usr/bin/pgrep"),
            &[
                "-P".as_ref(),
                "1".as_ref(),
                "-x".as_ref(),
                "osqueryd".as_ref(),
            ],
            CommandIo::Inspection {
                merge_stderr: false,
            },
        )?;
        match output.exit {
            1 => Ok(None),
            0 => {
                let first = output
                    .bytes
                    .split(|byte| *byte == b'\n')
                    .next()
                    .unwrap_or_default();
                std::str::from_utf8(first)
                    .ok()
                    .and_then(ParentPid::parse)
                    .map(Some)
                    .ok_or(InspectionFailure::Failed)
            }
            _ => Err(InspectionFailure::Failed),
        }
    }
}

#[cfg(test)]
mod tests;
