use super::{Configuration, Failure, reporting};
use posture_adapters::{
    ConvergeInstaller, DesiredStaging, InstalledTree, LibprocProcesses, OsqueryParents,
    OsqueryRestart, RestartTimer, SystemRunner, resolve_osqueryctl, resolve_osqueryi,
};
use posture_application::{converge, restart_daemon};
use std::{io::Write, time::Duration};

const COMMAND_BUDGET: Duration = Duration::from_secs(10);
pub(super) fn run(config: &Configuration, stdout: &mut impl Write) -> Result<(), Failure> {
    let command = resolve_osqueryctl(config.osqueryctl.as_deref(), &config.search_path)
        .map_err(Failure::Command)?;
    let osqueryi = resolve_osqueryi(config.osqueryd.as_deref(), &config.search_path)
        .map_err(Failure::Command)?;
    let Some(command) = command else {
        return Ok(());
    };
    let staging = DesiredStaging::new(config.desired.clone(), std::env::temp_dir());
    let mut live = InstalledTree::new(config.target.clone());
    let mut install = ConvergeInstaller::new(
        SystemRunner::per_command(COMMAND_BUDGET),
        config.sudo.clone(),
        config.target.clone(),
        config.log_directory.clone(),
    );
    let mut control = OsqueryRestart::new(
        SystemRunner::per_command(COMMAND_BUDGET),
        config.sudo.clone(),
        command,
        osqueryi,
        config.target.clone(),
    );
    let mut processes = OsqueryParents::new(LibprocProcesses::default());
    let mut clock = RestartTimer::default();
    converge(
        &staging,
        &mut live,
        &mut install,
        || restart_daemon(&mut control, &mut processes, &mut clock, config.bounds),
        |event| reporting::event(event, config, stdout),
    )
    .map_err(Failure::Converge)
}
