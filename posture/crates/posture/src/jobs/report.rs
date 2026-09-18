//! What each job looks like now beside what posture would install, rendered
//! as text. Pure in its readings, so the whole report is testable without a
//! launchd or a home directory.

use posture_domain::{JobPlan, UnitDrift, unit_drift};
use std::io::Write;
use std::path::PathBuf;

/// One job as it stands: what posture would write, what is on disk, and
/// whether launchd holds it.
pub(super) struct Reading {
    pub(super) plan: JobPlan,
    pub(super) path: PathBuf,
    /// The unit on disk, or `None` when there is no file to read.
    pub(super) unit: Option<String>,
    pub(super) loaded: bool,
}

impl Reading {
    /// How the unit on disk differs, which is nothing at all when it matches
    /// and everything when it is absent.
    fn drift(&self) -> Option<UnitDrift> {
        self.unit
            .as_ref()
            .map(|unit| unit_drift(&self.plan.unit(), unit))
    }

    /// Whether this job is installed, loaded and unchanged, which is the whole
    /// of what `verify` asserts.
    pub(super) fn is_ready(&self) -> bool {
        self.loaded && self.drift().is_some_and(|drift| drift.is_empty())
    }
}

/// Every job, one block each, with the differing lines named when `detail` is
/// set. The count of jobs that are not installed, loaded and unchanged is
/// returned so the caller can decide an exit code.
pub(super) fn render(
    readings: &[Reading],
    detail: bool,
    out: &mut impl Write,
) -> std::io::Result<usize> {
    let mut unready = 0;
    for reading in readings {
        let drift = reading.drift();
        let unit = match &drift {
            None => "no unit is installed at this path".to_string(),
            Some(drift) if drift.is_empty() => "matches what posture would write".to_string(),
            Some(drift) => format!(
                "differs from what posture would write in {} line(s)",
                drift.expected.len() + drift.live.len()
            ),
        };
        unready += usize::from(!reading.is_ready());
        writeln!(out, "{} {}", reading.plan.subcommand, reading.plan.label)?;
        writeln!(out, "  schedule  {}", reading.plan.schedule())?;
        writeln!(out, "  unit      {}: {unit}", reading.path.display())?;
        writeln!(
            out,
            "  launchd   {}",
            if reading.loaded {
                "loaded"
            } else {
                "not loaded"
            }
        )?;
        if let (true, Some(drift)) = (detail, drift) {
            for line in &drift.expected {
                writeln!(out, "  posture would write  {line}")?;
            }
            for line in &drift.live {
                writeln!(out, "  on disk only         {line}")?;
            }
        }
    }
    writeln!(
        out,
        "{} of {} jobs are installed, loaded and unchanged",
        readings.len() - unready,
        readings.len()
    )?;
    Ok(unready)
}
