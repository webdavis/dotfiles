//! `posture doctor`: can this machine deliver a page at all.
//!
//! ONE QUESTION, ASKED ON DEMAND. Every other surface reports that a job RAN.
//! A delivery config this build cannot use takes every destination away while
//! each job keeps reporting healthy, and before this subcommand the only record
//! of that was a line in a log nobody reads, so the state a security monitor
//! can least afford was the one nothing could be asked about.

use posture_adapters::{Notify, config_path};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn run(
    args: &[std::ffi::OsString],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    if !args.is_empty() {
        let _ = stderr.write_all(crate::USAGE.as_bytes());
        return 2;
    }
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        let _ = writeln!(stderr, "posture doctor: HOME is not set");
        return 1;
    };
    report(&Notify::read(&home), &config_path(&home), stdout)
}

/// The report, and 1 when nothing can be delivered. Pure in both arguments, so
/// the verdict is testable without a machine's own config file.
fn report(notify: &Notify, path: &Path, stdout: &mut impl Write) -> u8 {
    let _ = writeln!(
        stdout,
        "posture doctor: delivery config: {}",
        path.display()
    );
    for warning in &notify.warnings {
        let _ = writeln!(stdout, "posture doctor: warning: {warning}");
    }
    match &notify.refusal {
        Some(refusal) => {
            let _ = writeln!(
                stdout,
                "posture doctor: FAILED: no page can be delivered: {refusal}"
            );
            1
        }
        None => {
            let missing = notify.mode.missing_hermes_keys();
            if missing.is_empty() {
                let _ = writeln!(stdout, "posture doctor: the delivery config is usable");
                0
            } else {
                let reasons = missing
                    .iter()
                    .map(|route| {
                        format!("hermes mode names no signing key for the \"{route}\" route")
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let _ = writeln!(
                    stdout,
                    "posture doctor: FAILED: no page can be delivered: {reasons}"
                );
                1
            }
        }
    }
}

#[cfg(test)]
mod tests;
