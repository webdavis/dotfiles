mod allowlist;
mod converge;
use posture_adapters::{SystemInspection, SystemRunner};
use posture_application::{EnrichmentInspection, enrich};
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const USAGE: &str = "usage: posture <subcommand> [args]
  alert | poll | funnel | watchdog | digest | heartbeat | converge
  allowlist add <label> | allowlist deny <label> | allowlist list
  enrich <path>
  ssh install|verify|reload|rollback|print-config|print-path
only enrich, allowlist and converge are implemented; other subcommands exit 2
";

pub fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args.first().is_some_and(|word| word == "converge") {
        return converge::run(&args[1..], stdout, stderr);
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

#[cfg(test)]
mod tests;
