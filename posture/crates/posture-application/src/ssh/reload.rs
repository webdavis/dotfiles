use super::{
    SshBannerProbe, SshInstallFiles, SshLaunchctl, SshOutput, SshTree, SshVerification, Sshd,
};
use super::{SshFile, succeeded, verify::command_failure};
use posture_domain::{ReadinessRefusal, SshReadiness, SshRecord, compare_ssh_trees, has_host_key};
use std::time::Duration;
mod preflight;

pub struct SshReload<'a> {
    pub files: &'a mut dyn SshInstallFiles,
    pub sshd: &'a mut dyn Sshd,
    pub tree: &'a dyn SshTree,
    pub launchctl: &'a mut dyn SshLaunchctl,
    pub banners: &'a mut dyn SshBannerProbe,
    pub verify: &'a mut dyn FnMut() -> SshVerification,
    pub pause: &'a mut dyn FnMut(Duration),
}
pub fn reload_ssh(
    ports: &mut SshReload<'_>,
    readiness: Result<SshReadiness, ReadinessRefusal>,
    output: &mut SshOutput<'_>,
) -> u8 {
    let readiness = match readiness {
        Ok(value) => value,
        Err(error) => {
            return output.fail(&format!(
                "invalid readiness setting: {error:?}; sshd was not touched."
            ));
        }
    };
    let (before, probe_ports) = match preflight::run(ports, output) {
        Ok(ready) => ready,
        Err(message) => return output.fail(&format!("{message} sshd was not touched.")),
    };
    let loaded = ports.launchctl.probe();
    if matches!(&loaded, Ok(completed) if completed.status == 113) {
        return absent(ports.tree, &before, output);
    }
    if !succeeded(&loaded) {
        return output.fail(&format!("could not determine the state of the sshd launchd service: {}; neither loaded nor confirmed absent. sshd was not touched.", command_failure(loaded)));
    }
    let before_restart = match ports.tree.observe() {
        Ok(tree) => tree,
        Err(error) => return output.fail(&format!("the configuration tree could not be re-read before restart ({}); sshd was not touched.", super::verify::tree_failure(error))),
    };
    let changes = compare_ssh_trees(&before, &before_restart);
    if !changes.is_empty() {
        return output.fail(&format!("the configuration tree CHANGED while validation was running; sshd was not touched. What moved: {}. Re-run 'posture ssh reload' once the tree has settled.", preflight::changes(&changes)));
    }
    let recovery = recovery(ports.files);
    if !output.info(&format!("reload: about to restart sshd; on a remote machine this can drop the SSH session carrying this output. {recovery}")) { return 1; }
    let restarted = ports.launchctl.restart();
    if !succeeded(&restarted) {
        return output.fail(&format!("launchctl kickstart failed: {}; sshd may be in any state between untouched and stopped. {recovery}", command_failure(restarted)));
    }
    let loaded = ports.launchctl.probe();
    if !succeeded(&loaded) {
        return output.fail(&format!(
            "the sshd service did not reload: {} after kickstart. {recovery}",
            command_failure(loaded)
        ));
    }
    let Some(port) = banner(ports, &readiness, &probe_ports) else {
        let ports = probe_ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        return output.fail(&format!("POSSIBLE LOCKOUT: the launchd job reports loaded, but no SSH banner arrived on port(s) {ports} after {} attempt(s). On macOS, launchd owns Remote Login's listening socket, and sshd's Port directive does not move it; the daemon may be healthy on that socket (normally 22). Check 'ssh-keyscan -p 22 127.0.0.1' before treating this as a lockout. {recovery}", readiness.attempts));
    };
    let after = match ports.tree.observe() {
        Ok(tree) => tree,
        Err(error) => return output.fail(&format!("sshd RESTARTED and answered on port {port}, but the tree could not be re-read ({}). What the daemon read is unknown and no success is claimed. Nothing was rolled back. Check 'posture ssh verify'. {recovery}", super::verify::tree_failure(error))),
    };
    let changes = compare_ssh_trees(&before_restart, &after);
    if !changes.is_empty() {
        return output.fail(&format!("sshd RESTARTED and answered on port {port}, but the tree CHANGED after the last pre-restart check. What the daemon read is unknown. Nothing was rolled back. What moved: {}. Check 'posture ssh verify'. {recovery}", preflight::changes(&changes)));
    }
    u8::from(!output.info(&format!("reload complete: sshd restarted and is accepting connections on port {port} (SSH banner exchange completed), and every file in the configuration tree read back byte-for-byte and mode-for-mode identical each time this run read it, before the restart and after it. What the daemon read at its own instant is not observable from here, so that is a check that found nothing, not a guarantee.")))
}

fn absent(tree: &dyn SshTree, before: &[SshRecord], output: &mut SshOutput<'_>) -> u8 {
    if !output.info("reload: the sshd launchd service is confirmed absent; nothing was restarted.")
    {
        return 1;
    }
    let after = match tree.observe() {
        Ok(reading) => reading,
        Err(error) => return output.fail(&format!("could not re-read the configuration after the absent-service probe ({}); no claim is made about a future start.", super::verify::tree_failure(error))),
    };
    let changes = compare_ssh_trees(before, &after);
    if changes.is_empty() {
        u8::from(!output.info("reload: the configuration read back unchanged after preflight; a next start can use this validated on-disk configuration if it stays unchanged. Nothing is claimed about a running daemon."))
    } else {
        output.warning(&format!("the configuration CHANGED during validation: {}. Nothing was restarted and no claim is made about a future start.", preflight::changes(&changes)));
        0
    }
}

fn recovery(files: &dyn SshInstallFiles) -> String {
    format!(
        "Keep this SSH session OPEN until a second session connects. Keep physical-console or Screen Sharing access over the tailnet. If locked out, run 'posture ssh rollback' from that recovery session (or 'sudo rm {}'), then toggle Remote Login off and on in System Settings > General > Sharing.",
        files.path(SshFile::Target).display()
    )
}

fn banner(ports: &mut SshReload<'_>, readiness: &SshReadiness, probe_ports: &[u16]) -> Option<u16> {
    for attempt in 0..readiness.attempts {
        for &port in probe_ports {
            if let Ok(completed) = ports.banners.probe(port, readiness.probe_timeout)
                && completed.status == 0
                && has_host_key(&completed.output)
            {
                return Some(port);
            }
        }
        if attempt + 1 < readiness.attempts && !readiness.interval.is_zero() {
            (ports.pause)(readiness.interval);
        }
    }
    None
}

#[cfg(test)]
mod tests;
