use crate::{CommandIo, CommandRunner, SystemRunner, probes::POLL_PROBE_BUDGET};
use std::ffi::OsStr;
mod projection;
use posture_application::InspectionFailure;
use posture_domain::TrioReading;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub struct PostureTrio {
    pub values: [String; 3],
    pub exit: i32,
}
impl PostureTrio {
    pub fn reading(&self) -> TrioReading<'_> {
        TrioReading {
            values: self.values.each_ref().map(String::as_str),
            exit: self.exit,
        }
    }
}
pub struct PostureQuery<R = SystemRunner> {
    runner: R,
    program: PathBuf,
}
impl PostureQuery<SystemRunner> {
    pub fn new(program: PathBuf) -> Self {
        Self {
            runner: SystemRunner::per_command(POLL_PROBE_BUDGET),
            program,
        }
    }
}
impl<R: CommandRunner> PostureQuery<R> {
    pub fn read(&mut self) -> Result<PostureTrio, InspectionFailure> {
        let completed = self.runner.run_completed(
            &self.program,
            &[OsStr::new("--json"), OsStr::new(QUERY)],
            CommandIo::Inspection {
                merge_stderr: false,
            },
        )?;
        Ok(PostureTrio {
            values: if completed.exit == 0 {
                projection::values(&completed.bytes).unwrap_or_default()
            } else {
                Default::default()
            },
            exit: completed.exit,
        })
    }
}
const QUERY: &str = r"
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper,
    (SELECT enabled FROM screenlock) AS screenlock
";

#[cfg(test)]
mod tests;
