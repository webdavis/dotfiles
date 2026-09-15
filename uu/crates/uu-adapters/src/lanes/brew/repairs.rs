//! The three steps that are more than a command line: the two repairs an
//! upgrade makes necessary, and the App Store declarations the apply
//! deliberately does not run.

use std::fs;
use std::path::Path;
use std::time::Duration;

use super::steps::{bounded_step, note};
use crate::config::BrewLane;
use crate::lanes::CommandRunner;
use uu_domain::LaneReport;

/// The root-owned copy the system daemon runs. `brew upgrade` moves the
/// Homebrew build and never touches this one, so an upgraded tailscale would
/// otherwise leave the daemon on the old binary indefinitely.
const SYSTEM_TAILSCALED: &str = "/usr/local/bin/tailscaled";

/// Absolute, because a stripped launchd PATH does not carry /usr/bin.
const CMP: &str = "/usr/bin/cmp";
const SUDO: &str = "/usr/bin/sudo";

/// Re-install the system daemon when `brew upgrade` moved the binary under it.
///
/// GUARDED, so an ordinary week makes no privileged call and does not restart
/// the VPN. The comparison runs as `cmp` rather than reading both binaries
/// into this process, and sudo is passwordless here by the operator's own
/// config; if that ever changes the step reports the refusal like any other.
pub fn refresh_tailscaled(report: &mut LaneReport, runner: &dyn CommandRunner, lane: &BrewLane) {
    const LABEL: &str = "tailscaled refresh (if upgraded)";
    if !Path::new(&lane.tailscaled).exists() {
        report.noted(format!(
            "{LABEL}: tailscale is not installed here, so there is nothing to refresh"
        ));
        return;
    }
    if runner
        .run(CMP, &["-s", &lane.tailscaled, SYSTEM_TAILSCALED])
        .is_ok()
    {
        report.noted(format!(
            "{LABEL}: the system daemon already runs this build"
        ));
        return;
    }
    note(
        report,
        LABEL,
        runner.run(SUDO, &["-n", &lane.tailscaled, "install-system-daemon"]),
    );
}

/// Put our files back into /var/osquery if the osquery cask upgrade wiped
/// them, and restart the daemon if it did.
///
/// THIS LANE IS THE ONLY THING ON THE MACHINE THAT UPGRADES THAT CASK, and it
/// runs with nobody present, so without this step the machine could sit for a
/// week running a root daemon with no detection config and nothing would say
/// so.
///
/// A TOOL THAT IS NOT DEPLOYED IS A FAILED STEP, never a warning the run walks
/// past. Recording it as ok would advance the last-success marker over a week
/// in which the cask wiped /var/osquery and nothing put it back, which reads
/// in the record as a clean week. Weekly noise until an apply is run is the
/// point, not a side effect.
pub fn converge_osquery(report: &mut LaneReport, runner: &dyn CommandRunner, lane: &BrewLane) {
    const LABEL: &str = "osquery config converge (after upgrade)";
    const CONSEQUENCE: &str = "/var/osquery was NOT converged after this upgrade and the osquery \
                               configuration may be the vendor default; run chezmoi apply";
    let Some((program, arguments)) = lane.osquery_converge.split_first() else {
        report.failed(format!(
            "{LABEL}: no `osquery_converge` is configured, so {CONSEQUENCE}"
        ));
        return;
    };
    let args: Vec<_> = arguments.iter().map(String::as_str).collect();
    match runner.run(program, &args) {
        Ok(_) => report.noted(format!("{LABEL}: ok")),
        Err(why) => report.failed(format!("{LABEL}: {why}; {CONSEQUENCE}")),
    }
}

/// Install newly declared App Store apps from the manifest the apply publishes
/// and deliberately does not run, because the store wedges interactive runs.
///
/// NO MANIFEST IS THE ORDINARY STATE and a clean skip: nothing to install is
/// not a failure.
pub fn mas_declarations(
    report: &mut LaneReport,
    runner: &dyn CommandRunner,
    lane: &BrewLane,
    most: Duration,
) {
    const LABEL: &str = "mas declarations (bounded)";
    if !holds_something(&lane.mas_manifest) {
        report.noted(format!(
            "{LABEL}: no mas manifest published; nothing to install"
        ));
        return;
    }
    bounded_step(
        report,
        runner,
        LABEL,
        &lane.brew,
        &[
            "bundle",
            "--no-upgrade",
            &format!("--file={}", lane.mas_manifest),
        ],
        most,
    );
}

/// Whether a path names a regular file with anything in it. An empty manifest
/// declares nothing, so it reads the same as an absent one.
fn holds_something(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|found| found.is_file() && found.len() > 0)
}

#[cfg(test)]
pub(crate) mod tests;
