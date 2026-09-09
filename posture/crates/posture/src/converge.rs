mod configuration;
mod native;
mod reporting;
use configuration::{Configuration, ConfigurationRefusal};
use posture_adapters::CommandRefusal;
use posture_application::ConvergeFailure;
use std::{ffi::OsString, io::Write};

#[derive(Debug)]
enum Failure {
    Command(CommandRefusal),
    Converge(ConvergeFailure),
}

pub(super) fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    execute(
        args,
        Configuration::read(|name| std::env::var_os(name)),
        native::run,
        stdout,
        stderr,
    )
}

fn execute<W: Write>(
    args: &[OsString],
    config: Result<Configuration, Vec<ConfigurationRefusal>>,
    perform: impl FnOnce(&Configuration, &mut W) -> Result<(), Failure>,
    stdout: &mut W,
    stderr: &mut impl Write,
) -> u8 {
    let config = match config {
        Ok(config) => config,
        Err(errors) => {
            let _ = reporting::configuration(&errors, stderr);
            return 2;
        }
    };
    if let Some(argument) = args.first() {
        let _ = writeln!(stderr, "usage: posture converge");
        let _ = writeln!(
            stderr,
            "osquery-converge: unknown argument: {}",
            argument.to_string_lossy()
        );
        return 2;
    }
    match perform(&config, stdout) {
        Ok(()) => 0,
        Err(error) => {
            let _ = reporting::failure(&error, &config, stderr);
            1
        }
    }
}

#[cfg(test)]
mod tests;
