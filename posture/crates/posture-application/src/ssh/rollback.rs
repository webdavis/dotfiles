use super::{SshFile, SshInstallFiles, SshOutput, SshVerifyContext, Sshd};
use posture_domain::{PasswordChannel, password_channel};

pub fn rollback_ssh(
    files: &mut impl SshInstallFiles,
    sshd: &mut impl Sshd,
    context: &SshVerifyContext<'_>,
    output: &mut SshOutput<'_>,
) -> u8 {
    let target = files.path(SshFile::Target);
    let target = target.display();
    match files.exists(SshFile::Target) {
        Ok(false) => {
            if !output.info(&format!(
                "rollback: {target} is already absent; nothing to remove."
            )) {
                return 1;
            }
        }
        Ok(true) => {
            if !super::succeeded(&files.prime()) {
                return output
                    .fail("privilege escalation is unavailable; the drop-in was not removed.");
            }
            if !super::succeeded(&files.remove(&[SshFile::Target])) {
                return output.fail(&format!("could not remove '{target}'; the hardening is still in place. Remove it by hand (sudo rm {target}) and re-run 'posture ssh rollback' to confirm."));
            }
            if files.exists(SshFile::Target) != Ok(false) {
                return output.fail(&format!("'{target}' still exists or cannot be checked after the removal command reported success; refusing to claim the hardening is gone"));
            }
            if !output.info(&format!("rollback: removed {target}")) {
                return 1;
            }
        }
        Err(_) => return output.fail(&format!(
            "could not determine whether '{target}' exists; refusing to claim the hardening is gone"
        )),
    }
    if !sshd.available() {
        return if context.allow_missing {
            u8::from(!output.info(&format!("rollback: {target} is absent, but verification was SKIPPED via the test seam; whether password access is restored was NOT checked.")))
        } else {
            output.fail(&format!("'{target}' is absent, but '{}' cannot run, so whether password access is really restored cannot be checked; failing closed", context.executable.display()))
        };
    }
    match recovery(sshd, (context.user)().as_deref()) {
        PasswordChannel::Open => u8::from(!output.info(&format!("rollback complete: {target} is absent and an interactive password channel (PasswordAuthentication or KbdInteractiveAuthentication) resolves ON for the sampled loopback and off-loopback connections, so password access is restored at the next sshd start. The running daemon keeps its current configuration until sshd restarts: toggle Remote Login off and back on in System Settings > General > Sharing (or reboot). 'posture ssh reload' cannot perform this restart, because it refuses to restart onto a tree that is no longer hardened; reinstall first if the hardened policy should return."))),
        PasswordChannel::Blocked => output.fail(&format!("'{target}' is absent, but both interactive password channels still resolve OFF for a sampled connection, so another file is enforcing the policy and password access is NOT restored. Inspect the remaining configuration.")),
        PasswordChannel::Unreadable => output.fail("could not verify that password access is restored: the recovery check errored instead of answering (a resolution failed, reached its bound, or could not be parsed); refusing to guess. The removal itself already happened, so re-run 'posture ssh rollback' once the tree can be resolved."),
    }
}

fn recovery(sshd: &mut impl Sshd, user: Option<&str>) -> PasswordChannel {
    let Some(user) = user.filter(|user| !user.is_empty()) else {
        return PasswordChannel::Unreadable;
    };
    for (host, address) in [
        ("localhost", "127.0.0.1"),
        ("recovery.invalid", "198.51.100.23"),
    ] {
        let spec = format!("user={user},host={host},addr={address}");
        let Ok(completed) = sshd.connection(&spec) else {
            return PasswordChannel::Unreadable;
        };
        if completed.status != 0 {
            return PasswordChannel::Unreadable;
        }
        match password_channel(&completed.output) {
            PasswordChannel::Open => {}
            other => return other,
        }
    }
    PasswordChannel::Open
}

#[cfg(test)]
mod tests;
