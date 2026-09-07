mod configuration;
mod output;
use configuration::Configuration;
use posture_adapters::{AllowlistFile, AllowlistPublisher, AllowlistWriteLock, SystemLaunchdTable};
use posture_application::{AllowlistCommand, CurateAllowlist, CurationFailure, CurationOutcome};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

// Inspection and source lookup share the existing ten-second child-process policy. Publication
// gets one total budget, including terminal password waits, source lookup and manifest refresh.
const INSPECTION_BUDGET: Duration = Duration::from_secs(10);
const PUBLICATION_BUDGET: Duration = Duration::from_secs(15 * 60);
pub(super) fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let config = Configuration::read(|name| std::env::var_os(name));
    let mut launchd = SystemLaunchdTable::new(config.osqueryi, INSPECTION_BUDGET);
    let mut source = AllowlistFile::new(
        config.deployed.clone(),
        config.chezmoi.clone(),
        INSPECTION_BUDGET,
    );
    let mut publisher = AllowlistPublisher::new(
        config.chezmoi,
        config.deployed.clone(),
        config.manifest,
        PUBLICATION_BUDGET,
    );
    let lock = AllowlistWriteLock::new(&config.deployed);
    let mut curation = CurateAllowlist {
        home: &config.home,
        launchd: &mut launchd,
        source: &mut source,
        publisher: &mut publisher,
        lock: &lock,
    };
    execute(
        args,
        &config.deployed,
        |command| curation.run(command),
        stdout,
        stderr,
    )
}
fn execute(
    args: &[OsString],
    deployed: &Path,
    run: impl FnOnce(AllowlistCommand<'_>) -> Result<CurationOutcome, CurationFailure>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let label = args.get(1).map(|label| label.to_string_lossy());
    let command = match (
        args.first().and_then(|word| word.to_str()),
        label.as_deref(),
    ) {
        (Some("add"), Some(label)) => AllowlistCommand::Add(label),
        (Some("deny"), Some(label)) => AllowlistCommand::Deny(label),
        (Some("list"), _) => AllowlistCommand::List,
        _ => {
            let _ = stderr.write_all(crate::USAGE.as_bytes());
            return 2;
        }
    };
    match run(command) {
        Ok(outcome) => u8::from(output::success(outcome, stdout).is_err()),
        Err(CurationFailure::InvalidLabel(_))
            if args.get(1).is_some_and(|label| label.to_str().is_none()) =>
        {
            let _ = stderr.write_all(b"refused (invalid or system label): ");
            if let Some(label) = args.get(1) {
                let _ = stderr.write_all(label.as_bytes());
            }
            let _ = stderr.write_all(b"\n");
            1
        }
        Err(error) => {
            let _ = output::failure(error, deployed, label.as_deref().unwrap_or(""), stderr);
            1
        }
    }
}
#[cfg(test)]
mod tests;
