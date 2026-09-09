use posture_application::StagingRefusal;
use std::io::{self, Write};

pub(super) fn write(errors: &[StagingRefusal], output: &mut impl Write) -> io::Result<()> {
    for error in errors {
        match error {
            StagingRefusal::RelativeDirectory(path) => writeln!(
                output,
                "osquery-converge: the desired-state directory {} is not absolute; refusing.",
                path.display()
            )?,
            StagingRefusal::SymlinkComponent(path) => writeln!(
                output,
                "osquery-converge: the desired-state directory is reached through symlink {}; refusing to follow it.",
                path.display()
            )?,
            StagingRefusal::MissingDirectory(path) => writeln!(
                output,
                "osquery-converge: the desired state is not deployed at {}; run a full 'chezmoi apply'.",
                path.display()
            )?,
            StagingRefusal::IncompleteListing(path) => writeln!(
                output,
                "osquery-converge: the desired-state tree at {} could not be listed completely; refusing.",
                path.display()
            )?,
            StagingRefusal::SymlinkEntry(path) => writeln!(
                output,
                "osquery-converge: {} in the desired-state tree is a symlink; refusing to install through it.",
                path.display()
            )?,
            StagingRefusal::UnlistedEntry(path) => writeln!(
                output,
                "osquery-converge: {} sits in the desired-state tree but is not one of the files this tool installs, so it would be ignored forever; remove it, or deliberately list it beside its entry in desired/osquery.conf.",
                path.display()
            )?,
            StagingRefusal::MissingFile(path) => writeln!(
                output,
                "osquery-converge: the desired state at {} is not deployed as a regular file; run a full 'chezmoi apply'.",
                path.display()
            )?,
            StagingRefusal::CopyFailed(path) => writeln!(
                output,
                "osquery-converge: the desired state at {} could not be copied into the private stage; refusing.",
                path.display()
            )?,
            StagingRefusal::PrivateDirectory(path) => writeln!(
                output,
                "osquery-converge: could not create a private staging copy beneath {}; refusing.",
                path.display()
            )?,
        }
    }
    Ok(())
}
