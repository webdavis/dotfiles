mod configuration;
use configuration::Configuration;
use posture_adapters::{
    CommandRunner, LastResortBanner, SnapshotsFile, SystemClock, SystemRunner, alert_sink,
};
use posture_application::{Clock, Heartbeat, IndependentAlarm, Submission};
use std::{io::Write, time::Duration};

const PRODUCER_BUDGET: Duration = Duration::from_secs(5);
const ALARM_BUDGET: Duration = Duration::from_secs(10);
const INVALID_BOUND: &str =
    "posture heartbeat: invalid OSQUERY_CANARY_MAX_AGE literal; using 1800 seconds\n";
/// The banner's title when the heartbeat itself could not be delivered.
const UNDELIVERED: &str = "posture heartbeat undelivered";
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
    let route = config.notify.route.clone();
    // The banner is lent to the sink and taken back, so the run reports on the
    // one surface that needs no delivery even after every destination refused.
    let mut banner = LastResortBanner::new(alarm, config.alarm);
    let submission = {
        let sink = alert_sink(config.notify, runner, &mut banner, &mut *stderr);
        Heartbeat {
            clock,
            snapshots: SnapshotsFile::new(config.snapshots),
            sink,
            maximum_age: config.maximum_age,
        }
        .run()
    };
    let Submission::NotAccepted(failure) = submission else {
        return 0;
    };
    // AN UNDELIVERED HEARTBEAT PROVES NOTHING. Exiting 0 here would make
    // launchd's last exit code read as evidence of a live pipeline, which is
    // the one claim this job exists to make. A gateway outage lasting two
    // consecutive runs will also surface as a watchdog crash-loop page; that
    // is accepted, since the funnel already exits 1 on refusal under the
    // same watch.
    let report = format!("the heartbeat reached no destination (route {route}): {failure:?}");
    let _ = writeln!(stderr, "posture heartbeat: {report}");
    let _ = banner.alarm(UNDELIVERED, &report);
    1
}
#[cfg(test)]
mod tests;
