use super::{Configuration, ConfigurationRefusal, Failure};
use posture_application::ConvergeEvent;
use posture_domain::{CommandTrustRefusal, ConvergeDirectory};
use std::io::{self, Write};
mod failures;
mod staging;

pub(super) fn event(
    event: ConvergeEvent,
    config: &Configuration,
    output: &mut impl Write,
) -> io::Result<()> {
    match event {
        ConvergeEvent::DirectoryRepaired(directory, verdict) => {
            let target = match directory {
                ConvergeDirectory::Target => config.target.clone(),
                ConvergeDirectory::Packs => config.target.join("packs"),
            };
            writeln!(
                output,
                "osquery-converge: repaired {} ({})",
                target.display(),
                verdict.label()
            )
        }
        ConvergeEvent::FileInstalled(file, verdict) => writeln!(
            output,
            "osquery-converge: installed {} ({})",
            config.target.join(file.relative_path()).display(),
            verdict.label()
        ),
        ConvergeEvent::LogDirectoryCreated => writeln!(
            output,
            "osquery-converge: created {}",
            config.log_directory.display()
        ),
        ConvergeEvent::Restarted(restarted) => writeln!(
            output,
            "osquery-converge: restarted osqueryd (parent pid {}, still up after {}s)",
            restarted.parent.value(),
            restarted.settled_for.as_secs()
        ),
    }
}

pub(super) fn failure(
    error: &Failure,
    config: &Configuration,
    output: &mut impl Write,
) -> io::Result<()> {
    match error {
        Failure::Converge(error) => failures::write(error, config, output),
        Failure::Command(error) => {
            let command = error.command.display();
            match error.reason {
                CommandTrustRefusal::Relative => writeln!(
                    output,
                    "osquery-converge: the privileged command '{command}' did not resolve to an absolute path; refusing."
                ),
                CommandTrustRefusal::Unreadable => writeln!(
                    output,
                    "osquery-converge: the parent-directory attributes of {command} could not be read; refusing."
                ),
                CommandTrustRefusal::Owner(uid) => writeln!(
                    output,
                    "osquery-converge: {command} resolves inside a directory belonging to uid {uid} rather than uid 0; refusing to hand it to sudo."
                ),
                CommandTrustRefusal::Writable(mode) => writeln!(
                    output,
                    "osquery-converge: {command} resolves inside a directory whose mode {mode:04o} grants write to its group or to everyone; refusing."
                ),
            }
        }
    }
}

pub(super) fn configuration(
    errors: &[ConfigurationRefusal],
    output: &mut impl Write,
) -> io::Result<()> {
    for error in errors {
        match error {
            ConfigurationRefusal::UnexpectedOverride(name) => writeln!(
                output,
                "osquery-converge: {name} is a TEST-ONLY seam and is set in this environment, but OSQUERY_CONVERGE_TEST_SEAM=1 is not. It selects what root installs and where, so it is refused rather than honored; unset it."
            )?,
            ConfigurationRefusal::MissingSandboxPath(name) => writeln!(
                output,
                "osquery-converge: OSQUERY_CONVERGE_TEST_SEAM=1 is set but {name} is not, so it would fall back to its PRODUCTION default and this run would converge the live machine out of a test tree; refusing. A harness that engages the seam names its sandbox target directory AND its sudo."
            )?,
        }
    }
    Ok(())
}
