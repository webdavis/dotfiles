use super::{Configuration, staging};
use posture_application::{ConvergeFailure, ConvergeRefusal, RestartFailure, VendorPlist};
use std::io::{self, Write};

pub(super) fn write(
    error: &ConvergeFailure,
    config: &Configuration,
    output: &mut impl Write,
) -> io::Result<()> {
    match error {
        ConvergeFailure::Preparation(ConvergeRefusal::Staging(errors)) => {
            staging::write(errors, output)
        }
        ConvergeFailure::Preparation(ConvergeRefusal::IrregularDirectory(directory)) => writeln!(
            output,
            "osquery-converge: {} is not a directory: a symlink, a file or a device stands there. 'install -d' would FOLLOW it rather than replace it; refusing. Remove whatever is at that path by hand.",
            config.target.join(directory.relative_path()).display()
        ),
        ConvergeFailure::Directory(directory, cause) => writeln!(
            output,
            "osquery-converge: could not repair {} ({cause:?}); the restart was not attempted.",
            config.target.join(directory.relative_path()).display()
        ),
        ConvergeFailure::File(file, cause) => writeln!(
            output,
            "osquery-converge: could not install {} ({cause:?}); the restart was not attempted.",
            config.target.join(file.relative_path()).display()
        ),
        ConvergeFailure::LogDirectory(cause) => writeln!(
            output,
            "osquery-converge: could not create {} ({cause:?}); the restart was not attempted.",
            config.log_directory.display()
        ),
        ConvergeFailure::Report => writeln!(
            output,
            "osquery-converge: could not write the converge report; the run stopped."
        ),
        ConvergeFailure::Restart(error) => restart(error, config, output),
    }
}

fn restart(
    error: &RestartFailure,
    config: &Configuration,
    output: &mut impl Write,
) -> io::Result<()> {
    let vendor = config.target.join("io.osquery.agent.plist");
    match error {
        RestartFailure::VendorPlist(VendorPlist::Symlink) => writeln!(
            output,
            "osquery-converge: {} is a symlink, and 'osqueryctl start' copies that file into /Library/LaunchDaemons and loads it as root; refusing. That file belongs to the osquery package; remove the link and reinstall the cask. The daemon was NOT stopped.",
            vendor.display()
        ),
        RestartFailure::VendorPlist(_) => writeln!(
            output,
            "osquery-converge: {} is missing or not a regular file, so 'osqueryctl start' would have no LaunchDaemon plist to install. That file belongs to the osquery package; reinstall the cask. The daemon was NOT stopped.",
            vendor.display()
        ),
        RestartFailure::Configuration(_) => writeln!(
            output,
            "osquery-converge: the converged configuration at {} does not pass 'osqueryctl config-check'. The files are installed; the running daemon was NOT stopped and is still on its previous configuration.",
            config.target.join("osquery.conf").display()
        ),
        RestartFailure::Start(_) => writeln!(
            output,
            "osquery-converge: 'osqueryctl start' FAILED, so the restart could not be established. Diagnose with 'sudo osqueryctl status' and 'sudo launchctl print system/io.osquery.agent'."
        ),
        RestartFailure::ParentProbe(_) => writeln!(
            output,
            "osquery-converge: the osqueryd parent process could not be inspected, so the restart could not be established."
        ),
        RestartFailure::UnchangedParent(parent) => writeln!(
            output,
            "osquery-converge: osqueryd is still running as parent pid {}, the same process that was running before the stop, so it never restarted and is still on its PREVIOUS configuration. Diagnose with 'sudo launchctl print system/io.osquery.agent'.",
            parent.value()
        ),
        RestartFailure::Deadline => writeln!(
            output,
            "osquery-converge: osqueryd did not come back within {}s of 'osqueryctl start'. Diagnose with 'sudo osqueryctl status'.",
            config.bounds.deadline().as_secs()
        ),
        RestartFailure::UnstableParent(parent) => writeln!(
            output,
            "osquery-converge: osqueryd started (pid {}) and was gone again within {}s. KeepAlive will not retry for another 60 seconds (ThrottleInterval), so treat this machine as unmonitored until it is fixed.",
            parent.value(),
            config.bounds.settle().as_secs()
        ),
    }
}
