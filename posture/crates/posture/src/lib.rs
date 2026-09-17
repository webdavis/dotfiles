mod alert;
mod allowlist;
mod converge;
mod digest;
mod doctor;
mod funnel;
mod heartbeat;
mod jobs;
mod poll;
mod ssh;
mod watchdog;
use posture_adapters::{SystemInspection, SystemRunner};
use posture_application::{EnrichmentInspection, enrich};
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const USAGE: &str = "usage: posture <subcommand> [args]
  alert | poll | funnel | watchdog | digest | heartbeat | converge | doctor
  allowlist add <label> | allowlist deny <label> | allowlist list
  enrich <path>
  ssh install|verify|reload|rollback|print-config|print-path
  jobs install|verify|list|print <job>
only enrich, allowlist, converge, doctor, heartbeat, digest, alert, poll, funnel, ssh, jobs and
watchdog are implemented; other subcommands exit 2
";

pub fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args.first().is_some_and(|word| word == "poll") {
        return poll::run(stderr);
    }
    if args.first().is_some_and(|word| word == "ssh") {
        return ssh::run(&args[1..], stdout, stderr);
    }
    if args.first().is_some_and(|word| word == "jobs") {
        return jobs::run(&args[1..], stdout, stderr);
    }
    if args.first().is_some_and(|word| word == "converge") {
        return converge::run(&args[1..], stdout, stderr);
    }
    if args.first().is_some_and(|word| word == "funnel") {
        return funnel::run(stderr);
    }
    if args.first().is_some_and(|word| word == "watchdog") {
        return watchdog::run(&args[1..], stderr);
    }
    if args.first().is_some_and(|word| word == "heartbeat") {
        return heartbeat::run(stderr);
    }
    if args.first().is_some_and(|word| word == "digest") {
        return digest::run(stderr);
    }
    if args.first().is_some_and(|word| word == "alert") {
        return alert::run(stderr);
    }
    if args.first().is_some_and(|word| word == "doctor") {
        return doctor::run(&args[1..], stdout, stderr);
    }
    if args.first().is_some_and(|word| word == "allowlist") {
        return allowlist::run(&args[1..], stdout, stderr);
    }
    // One budget covers every spawned inspection for a finding, including plist fallback.
    let mut inspection = SystemInspection::new(SystemRunner::new(Duration::from_secs(10)));
    let status = execute(args, &mut inspection, stdout, stderr);
    let _ = stderr.write_all(inspection.diagnostics());
    status
}

fn execute(
    args: &[OsString],
    inspection: &mut impl EnrichmentInspection,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    if args.first().is_none_or(|word| word != "enrich") {
        let _ = stderr.write_all(USAGE.as_bytes());
        return 2;
    }
    // The Bash enricher treats absent/empty path as not applicable and ignores later operands.
    let path = args.get(1).map_or(Path::new(""), Path::new);
    let outcome = enrich(path, inspection);
    if stdout.write_all(&outcome.fact).is_err() {
        return 1;
    }
    outcome.exit_code()
}

/// A notify choice that hands every page to one owned fixture command, which
/// is how a command test drives the whole composition without a real engine.
#[cfg(test)]
pub(crate) fn command_notify(command: &Path) -> posture_adapters::Notify {
    posture_adapters::Notify {
        mode: posture_adapters::NotifyMode::Command {
            path: command.to_path_buf(),
            arguments: vec!["send".to_string(), "--json".to_string()],
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;
