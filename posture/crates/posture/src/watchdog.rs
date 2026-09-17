mod configuration;
use configuration::Configuration;
use posture_adapters::{
    CommandRunner, GatewayProbe, LastResortBanner, QueueDatabase, SnapshotsFile, SystemClock,
    SystemRunner, SystemWatchdogProcesses, WatchdogAudit, WatchdogStateFile, alert_sink,
};
use posture_application::{Clock, GatewayHealth, Watchdog, WatchdogOutcome};
use std::{ffi::OsString, io::Write, time::Duration};

pub(super) fn run(args: &[OsString], stderr: &mut impl Write) -> u8 {
    if !args.is_empty() {
        let _ = stderr.write_all(b"usage: posture watchdog\n");
        return 2;
    }
    let Some(config) = Configuration::read(|name| std::env::var_os(name)) else {
        let _ = stderr.write_all(b"posture watchdog: HOME is not set\n");
        return 1;
    };
    let mut gateway = GatewayProbe::new(config.gateway.clone(), config.route_timeout);
    execute(
        config,
        SystemClock,
        Runners {
            processes: SystemRunner::per_command(Duration::from_secs(3)),
            producer: SystemRunner::per_command(Duration::from_secs(5)),
            fallback: SystemRunner::per_command(Duration::from_secs(10)),
            independent: SystemRunner::per_command(Duration::from_secs(10)),
        },
        &mut gateway,
        stderr,
    )
}
struct Runners<R> {
    processes: R,
    producer: R,
    fallback: R,
    independent: R,
}
fn execute(
    config: Configuration,
    mut clock: impl Clock,
    runners: Runners<impl CommandRunner>,
    gateway: &mut dyn GatewayHealth,
    stderr: &mut impl Write,
) -> u8 {
    let mut sink = alert_sink(
        config.notify,
        runners.producer,
        LastResortBanner::new(runners.fallback, config.alarm.clone()),
        &mut *stderr,
    );
    let outcome = Watchdog {
        clock: &mut clock,
        snapshots: &mut SnapshotsFile::new(config.snapshots),
        processes: &mut SystemWatchdogProcesses::current_user(runners.processes),
        gateway,
        legacy_queue: &mut QueueDatabase::legacy(config.legacy_queue),
        pns_ledger: &mut QueueDatabase::pns(config.pns_ledger),
        integrity: &mut WatchdogAudit {
            pipeline: config.pipeline,
            managed_bin: config.managed_bin,
            pns: config.pns,
            authority: config.authority,
            bounds: config.bounds,
        },
        state: &mut WatchdogStateFile::new(config.state.clone()),
        sink: &mut sink,
        alarm: &mut LastResortBanner::new(runners.independent, config.alarm),
        maximum_age: config.maximum_age,
        agents: &config.agents,
        gateway_url: &config.gateway,
        state_path: &config.state.to_string_lossy(),
    }
    .run();
    let (status, diagnostic): (u8, &[u8]) = match outcome {
        WatchdogOutcome::Healthy | WatchdogOutcome::Reported => (0, b""),
        WatchdogOutcome::HealthyStateLost => (0, b"posture watchdog: could not persist healthy state\n"),
        WatchdogOutcome::DeliveryFailed => (1, b"posture watchdog: security page was not durably accepted; state not advanced\n"),
        WatchdogOutcome::IndependentAlarmFailed => (1, b"posture watchdog: independent pns alarm failed; state not advanced, retrying next tick\n"),
        WatchdogOutcome::StateFailed => (1, b"posture watchdog: page accepted but state publication failed; retrying next tick\n"),
    };
    let _ = stderr.write_all(diagnostic);
    status
}
#[cfg(test)]
mod tests;
