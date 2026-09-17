//! The live half of `posture jobs`: the units on disk and launchd itself.
//!
//! ONE SEAM IN FRONT OF BOTH. Every path hangs off the home directory handed
//! in and every launchctl call goes through a `CommandRunner`, so a test
//! drives a temporary directory and a recorded command list rather than the
//! launchd of the machine running the suite. Loading a real unit while
//! testing would fight whatever installed that machine's jobs.

use super::{Verb, report};
use posture_adapters::{
    CommandIo, CommandRunner, JobSettings, SystemRunner, current_uid, job_settings,
};
use posture_domain::JobPlan;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Each launchctl call is one short command, and a hung one must not hold the
/// whole report.
const LAUNCHCTL_BUDGET: Duration = Duration::from_secs(10);
const LAUNCHCTL: &str = "/bin/launchctl";

pub(super) fn run(verb: Verb, stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        let _ = stderr.write_all(b"posture jobs: HOME is not set\n");
        return 1;
    };
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(error) => {
            let _ = writeln!(
                stderr,
                "posture jobs: this program's own path is unreadable: {error}"
            );
            return 1;
        }
    };
    let mut jobs = Jobs::new(
        home,
        &program,
        SystemRunner::per_command(LAUNCHCTL_BUDGET),
        current_uid(),
    );
    perform(verb, &mut jobs, stdout, stderr)
}

/// Every job posture installs, for one home directory, with the seam its live
/// readings come through.
pub(super) struct Jobs<R> {
    home: PathBuf,
    plans: Vec<JobPlan>,
    settings: JobSettings,
    runner: R,
    uid: u32,
}

impl<R: CommandRunner> Jobs<R> {
    pub(super) fn new(home: PathBuf, program: &Path, runner: R, uid: u32) -> Self {
        let settings = job_settings(&home);
        let plans = JobPlan::all(&settings.labels, settings.daily, program, &home);
        Self {
            home,
            plans,
            settings,
            runner,
            uid,
        }
    }

    fn readings(&mut self) -> Vec<report::Reading> {
        let plans = std::mem::take(&mut self.plans);
        let readings = plans
            .iter()
            .map(|plan| {
                let path = plan.unit_path(&self.home);
                report::Reading {
                    plan: plan.clone(),
                    unit: std::fs::read_to_string(&path).ok(),
                    loaded: self.loaded(&plan.label),
                    path,
                }
            })
            .collect();
        self.plans = plans;
        readings
    }

    /// Whether launchd holds the label in this user's own domain. A refusal
    /// is an unloaded job: `launchctl print` exits nonzero for a label it does
    /// not know.
    fn loaded(&mut self, label: &str) -> bool {
        self.launchctl(&[
            OsStr::new("print"),
            OsStr::new(&format!("gui/{}/{label}", self.uid)),
        ])
        .is_ok()
    }

    fn launchctl(&mut self, args: &[&OsStr]) -> Result<Vec<u8>, ()> {
        self.runner
            .run(
                Path::new(LAUNCHCTL),
                args,
                CommandIo::Inspection {
                    merge_stderr: false,
                },
            )
            .map_err(|_| ())
    }

    /// Write one unit and load it, replacing whatever was loaded under that
    /// label. The bootout is expected to fail for a job launchd does not hold,
    /// which is the ordinary first install.
    fn install(&mut self, plan: &JobPlan, out: &mut impl Write) -> Result<(), String> {
        let path = plan.unit_path(&self.home);
        for directory in [path.parent(), plan.log.parent()].into_iter().flatten() {
            std::fs::create_dir_all(directory)
                .map_err(|error| format!("{}: {error}", directory.display()))?;
        }
        std::fs::write(&path, plan.unit())
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let target = format!("gui/{}/{}", self.uid, plan.label);
        let _ = self.launchctl(&[OsStr::new("bootout"), OsStr::new(&target)]);
        let domain = format!("gui/{}", self.uid);
        self.launchctl(&[
            OsStr::new("bootstrap"),
            OsStr::new(&domain),
            path.as_os_str(),
        ])
        .map_err(|()| format!("launchctl could not load {}", plan.label))?;
        let _ = writeln!(
            out,
            "{}: wrote {} and loaded {}",
            plan.subcommand,
            path.display(),
            plan.label
        );
        Ok(())
    }
}

pub(super) fn perform<R: CommandRunner>(
    verb: Verb,
    jobs: &mut Jobs<R>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    for warning in &jobs.settings.warnings {
        let _ = writeln!(stderr, "posture jobs: {warning}");
    }
    match verb {
        Verb::Print(agent) => {
            let Some(plan) = jobs
                .plans
                .iter()
                .find(|plan| plan.subcommand == agent.key())
            else {
                return 1;
            };
            u8::from(stdout.write_all(plan.unit().as_bytes()).is_err())
        }
        Verb::List | Verb::Verify => {
            let readings = jobs.readings();
            match report::render(&readings, verb == Verb::Verify, stdout) {
                Err(_) => 1,
                Ok(_) if verb == Verb::List => 0,
                Ok(unready) => u8::from(unready > 0),
            }
        }
        Verb::Install => {
            let plans = std::mem::take(&mut jobs.plans);
            let mut failures = 0;
            for plan in &plans {
                if let Err(refusal) = jobs.install(plan, stdout) {
                    failures += 1;
                    let _ = writeln!(
                        stderr,
                        "posture jobs install: {}: {refusal}",
                        plan.subcommand
                    );
                }
            }
            jobs.plans = plans;
            u8::from(failures > 0)
        }
    }
}
