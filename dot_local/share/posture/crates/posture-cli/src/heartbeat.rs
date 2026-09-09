mod configuration;
use configuration::Configuration;
use posture_adapters::{
    CommandRunner, LastResortBanner, PnsProducer, SnapshotsFile, SystemClock, SystemRunner,
};
use posture_application::{Clock, Heartbeat};
use std::{io::Write, time::Duration};

const PRODUCER_BUDGET: Duration = Duration::from_secs(5);
const ALARM_BUDGET: Duration = Duration::from_secs(10);
const INVALID_BOUND: &str =
    "posture heartbeat: invalid OSQUERY_CANARY_MAX_AGE literal; using 1800 seconds\n";
pub(super) fn run(stderr: &mut impl Write) -> u8 {
    let Some(config) = Configuration::read(|name| std::env::var_os(name)) else {
        let _ = stderr.write_all(b"posture heartbeat: HOME is not set\n");
        return 1;
    };
    execute(
        config,
        SystemClock,
        SystemRunner::per_command(PRODUCER_BUDGET),
        SystemRunner::per_command(ALARM_BUDGET),
        stderr,
    )
}
fn execute(
    config: Configuration,
    clock: impl Clock,
    runner: impl CommandRunner,
    alarm: impl CommandRunner,
    stderr: &mut impl Write,
) -> u8 {
    if config.maximum_age.invalid_literal() {
        let _ = stderr.write_all(INVALID_BOUND.as_bytes());
    }
    let sink = PnsProducer::new(
        runner,
        config.pns,
        Some(
            String::from("posture")
                .try_into()
                .expect("the fixed posture route is valid"),
        ),
        LastResortBanner::new(alarm, config.alarm),
    );
    Heartbeat {
        clock,
        snapshots: SnapshotsFile::new(config.snapshots),
        sink,
        maximum_age: config.maximum_age,
    }
    .run();
    0
}
#[cfg(test)]
mod tests;
