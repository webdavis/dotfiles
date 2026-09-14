use super::*;
use posture_domain::{SshTreeChange, ssh_ports};

pub(super) fn run(
    reload: &mut SshReload<'_>,
    output: &mut SshOutput<'_>,
) -> Result<(Vec<SshRecord>, Vec<u16>), String> {
    let prime = reload.files.prime();
    if !succeeded(&prime) {
        return Err(format!(
            "privilege escalation is unavailable: {}. This is not a statement about the sshd service.",
            command_failure(prime)
        ));
    }
    if !reload.banners.available() {
        return Err("the readiness prover is not runnable; refusing to kickstart blind.".into());
    }
    let before = reload.tree.observe().map_err(|error| {
        format!(
            "could not read the configuration tree before validation: {}.",
            super::super::verify::tree_failure(error)
        )
    })?;
    let syntax = reload.sshd.syntax();
    if !succeeded(&syntax) {
        return Err(format!(
            "the configuration's syntax check did not pass: {}; refusing to restart onto it.",
            command_failure(syntax)
        ));
    }
    // A successful syntax check can still issue diagnostics, which must not disappear.
    if let Ok(completed) = syntax
        && !completed.output.is_empty()
    {
        let _ = output.stderr.write_all(&completed.output);
    }
    let verification = (reload.verify)();
    if !output.verification(&verification) {
        return Err("the effective configuration is not fully hardened (the verify failures are above); refusing to restart onto it.".into());
    }
    let completed = reload.sshd.global();
    if !succeeded(&completed) {
        return Err(format!(
            "could not resolve the effective sshd port: {}; refusing to restart blind.",
            command_failure(completed)
        ));
    }
    let completed = completed.map_err(|_| "could not read the effective sshd ports".to_owned())?;
    let ports = ssh_ports(&completed.output).map_err(|error| format!("the effective configuration did not resolve usable canonical ports: {error:?}; refusing to restart blind."))?;
    Ok((before, ports))
}

pub(super) fn changes(changes: &[SshTreeChange]) -> String {
    changes
        .iter()
        .map(|change| match change {
            SshTreeChange::Appeared(path) => {
                format!("'{}' appeared", String::from_utf8_lossy(path))
            }
            SshTreeChange::Disappeared(path) => {
                format!("'{}' disappeared", String::from_utf8_lossy(path))
            }
            SshTreeChange::Content(path) => {
                format!("'{}' content changed", String::from_utf8_lossy(path))
            }
            SshTreeChange::Attributes(path, before, after) => format!(
                "'{}' mode/owner/group changed from {:o}/{}/{} to {:o}/{}/{}",
                String::from_utf8_lossy(path),
                before.mode,
                before.uid,
                before.gid,
                after.mode,
                after.uid,
                after.gid
            ),
        })
        .collect::<Vec<_>>()
        .join("; ")
}
