use super::{
    SshCommandResult, SshFile, SshInstallFiles, SshOutput, SshVerification, succeeded,
    verify::command_failure,
};
use posture_domain::ssh_config;
mod restore;

pub trait SshInstallSignals {
    fn pending(&self) -> Option<i32>;
    fn defer(&mut self);
    fn disarm(&mut self);
}

pub fn install_ssh(
    files: &mut impl SshInstallFiles,
    verify: &mut impl FnMut() -> SshVerification,
    signals: &mut impl SshInstallSignals,
    output: &mut SshOutput<'_>,
) -> u8 {
    if !files.directory_exists() {
        return output.fail("the drop-in directory does not exist");
    }
    let mut state = Transaction::default();
    let result = transact(files, verify, signals, output, &mut state);
    let verification = match result {
        Ok(verification) => verification,
        Err(message) => {
            signals.defer();
            let restored = if state.begun {
                restore::run(files, state.replacement_started, output)
            } else {
                true
            };
            if let Some(signal) = signals.pending() {
                output.fail(&format!("INTERRUPTED (signal {signal}): no success is claimed and the owned command group was stopped before rollback."));
            }
            let restoration = if restored {
                "the tree was rolled back to the state this install found it in"
            } else {
                "rollback was incomplete; see the warnings above"
            };
            return output.fail(&format!(
                "{message}; {restoration}, and no success is claimed"
            ));
        }
    };
    signals.disarm();
    if !succeeded(&files.remove(&[SshFile::SavedTarget, SshFile::SavedLegacy])) {
        output.warning("could not remove the rollback copies; they are dot-prefixed and inert, but should be cleaned up");
    }
    let target = files.path(SshFile::Target);
    let message = match verification {
        SshVerification::Skipped => format!(
            "wrote {}, but verification was SKIPPED via the test seam; the effective configuration is NOT verified.",
            target.display()
        ),
        _ => format!(
            "install complete: {} is in place and the effective configuration verified fully hardened.",
            target.display()
        ),
    };
    u8::from(!output.info(&message))
}

#[derive(Default)]
struct Transaction {
    begun: bool,
    replacement_started: bool,
}

fn transact(
    files: &mut impl SshInstallFiles,
    verify: &mut impl FnMut() -> SshVerification,
    signals: &impl SshInstallSignals,
    output: &mut SshOutput<'_>,
    state: &mut Transaction,
) -> Result<SshVerification, String> {
    check(files.prime(), "prime privilege", signals)?;
    let clear = files.remove(&[SshFile::Staging, SshFile::SavedTarget, SshFile::SavedLegacy]);
    state.begun = succeeded(&clear);
    check(clear, "clear the working files; refusing to begin", signals)?;
    check(
        files.stage(ssh_config().as_bytes()),
        "stage the new drop-in",
        signals,
    )?;
    check(
        files.chmod(),
        "set mode 0644 on the staged drop-in",
        signals,
    )?;
    if files
        .exists(SshFile::Target)
        .map_err(|_| "could not inspect the existing target")?
    {
        check(
            files.save(),
            "save the existing target before replacing it",
            signals,
        )?;
    }
    // After a successful backup, a rename can take effect before its child reports failure.
    state.replacement_started = true;
    let published = files.rename(SshFile::Staging, SshFile::Target);
    check(published, "publish the staged drop-in", signals)?;
    if !output.info(&format!(
        "wrote {} (mode 0644)",
        files.path(SshFile::Target).display()
    )) {
        return Err("could not report the published drop-in".into());
    }
    if files
        .exists(SshFile::Legacy)
        .map_err(|_| "could not inspect the legacy drop-in")?
    {
        check(
            files.rename(SshFile::Legacy, SshFile::SavedLegacy),
            "move the legacy drop-in aside",
            signals,
        )?;
        if !output.info(&format!(
            "removed legacy drop-in {}",
            files.path(SshFile::Legacy).display()
        )) {
            return Err("could not report the moved legacy drop-in".into());
        }
    }
    let verification = verify();
    let reported = output.verification(&verification);
    if signals.pending().is_some() {
        return Err("install was interrupted during verification".into());
    }
    if !reported {
        return Err("the effective configuration did NOT verify as fully hardened".into());
    }
    Ok(verification)
}

fn check(
    result: SshCommandResult,
    step: &str,
    signals: &impl SshInstallSignals,
) -> Result<(), String> {
    if !succeeded(&result) {
        return Err(format!("could not {step}: {}", command_failure(result)));
    }
    if signals.pending().is_some() {
        return Err(format!(
            "install was interrupted after attempting to {step}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
