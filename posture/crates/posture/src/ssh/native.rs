use super::{Configuration, Verb};
use posture_adapters::{
    SshConfigTree, SshFileInstaller, SshKeyscan, SshLaunchd, SshSignals, SshdCommand, SystemRunner,
    current_user_name, ssh_install_cancelled,
};
use posture_application::SshOutput;
use posture_application::{
    SshReload, SshVerifyContext, install_ssh, reload_ssh, rollback_ssh, verify_ssh,
};

pub(super) fn run(verb: Verb, config: &Configuration, output: &mut SshOutput<'_>) -> u8 {
    perform(verb, config, &current_user_name, output)
}
fn perform(
    verb: Verb,
    config: &Configuration,
    lookup_user: &dyn Fn() -> Option<String>,
    output: &mut SshOutput<'_>,
) -> u8 {
    let tree = SshConfigTree::new(config.main.clone(), config.dropins.clone());
    let context = SshVerifyContext {
        user: lookup_user,
        executable: &config.sshd,
        allow_missing: config.allow_missing,
    };
    let mut verify = || {
        // Each verification owns one aggregate budget, starting after installation or syntax.
        let mut sshd = sshd(config, verification_runner(config));
        verify_ssh(&mut sshd, &tree, &context)
    };
    let mut files = SshFileInstaller::new(
        command_runner(config),
        config.dropins.clone(),
        config.sudo.clone(),
    );
    match verb {
        Verb::Verify => u8::from(!output.verification(&verify())),
        Verb::Rollback => rollback_ssh(
            &mut files,
            &mut sshd(config, command_runner(config)),
            &context,
            output,
        ),
        Verb::Install => {
            let mut signals =
                match SshSignals::arm() {
                    Ok(guard) => guard,
                    Err(error) => return output.fail(&format!(
                        "could not arm install rollback for signals: {error}; nothing was written"
                    )),
                };
            let status = install_ssh(&mut files, &mut verify, &mut signals, output);
            signals.reraise();
            status
        }
        Verb::Reload => {
            let readiness = config.readiness();
            let probe_budget = readiness.as_ref().map_or(config.deadline, |r| {
                std::time::Duration::from_secs(u64::from(r.probe_timeout)).min(config.deadline)
            });
            let mut banners = SshKeyscan::new(
                SystemRunner::per_command(probe_budget).with_termination_grace(config.grace),
                config.keyscan.clone(),
            );
            let mut launchctl = SshLaunchd::new(
                command_runner(config),
                config.launchctl.clone(),
                config.sudo.clone(),
            );
            reload_ssh(
                &mut SshReload {
                    files: &mut files,
                    sshd: &mut sshd(config, command_runner(config)),
                    tree: &tree,
                    launchctl: &mut launchctl,
                    banners: &mut banners,
                    verify: &mut verify,
                    pause: &mut std::thread::sleep,
                },
                readiness,
                output,
            )
        }
    }
}
fn verification_runner(config: &Configuration) -> SystemRunner {
    SystemRunner::new(config.deadline)
        .with_termination_grace(config.grace)
        .with_cancellation(ssh_install_cancelled)
}
fn command_runner(config: &Configuration) -> SystemRunner {
    SystemRunner::per_command(config.deadline)
        .with_termination_grace(config.grace)
        .with_cancellation(ssh_install_cancelled)
}

fn sshd(config: &Configuration, runner: SystemRunner) -> SshdCommand<SystemRunner> {
    SshdCommand::new(
        runner,
        config.sshd.clone(),
        config.main.clone(),
        config.sudo.clone(),
    )
}

#[cfg(test)]
mod tests;
