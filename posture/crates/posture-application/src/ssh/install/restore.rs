use super::*;

pub(super) fn run(
    files: &mut impl SshInstallFiles,
    replacement_started: bool,
    output: &mut SshOutput<'_>,
) -> bool {
    let mut restored = true;
    match files.exists(SshFile::Staging) {
        Ok(true) => {
            restored &= attempt(
                files.remove(&[SshFile::Staging]),
                "remove the staging file",
                output,
            )
        }
        Ok(false) => {}
        Err(_) => {
            output.warning("could not inspect the staging file during rollback");
            restored = false;
        }
    }
    if replacement_started {
        match files.exists(SshFile::SavedTarget) {
            Ok(true) => {
                restored &= attempt(
                    files.rename(SshFile::SavedTarget, SshFile::Target),
                    "restore the previous drop-in",
                    output,
                )
            }
            Ok(false) => {
                restored &= attempt(
                    files.remove(&[SshFile::Target]),
                    "remove the drop-in this run created",
                    output,
                )
            }
            Err(_) => {
                output.warning("could not inspect the saved target; refusing to remove the published drop-in without knowing whether a prior copy exists");
                restored = false;
            }
        }
    }
    match files.exists(SshFile::SavedLegacy) {
        Ok(true) => {
            restored &= attempt(
                files.rename(SshFile::SavedLegacy, SshFile::Legacy),
                "restore the legacy drop-in",
                output,
            )
        }
        Ok(false) => {}
        Err(_) => {
            output.warning("could not inspect the saved legacy drop-in during rollback");
            restored = false;
        }
    }
    restored
}
fn attempt(result: SshCommandResult, action: &str, output: &mut SshOutput<'_>) -> bool {
    if succeeded(&result) {
        true
    } else {
        output.warning(&format!(
            "rollback could not {action}: {}",
            command_failure(result)
        ));
        false
    }
}
