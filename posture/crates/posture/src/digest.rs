mod configuration;
use configuration::Configuration;
use posture_adapters::{
    CommandRunner, DigestSpoolFile, LastResortBanner, SystemClock, SystemRunner, alert_sink,
    prepare_spool_directory,
};
use posture_application::{BuildDigest, Clock, DigestOutcome};
use std::{io::Write, time::Duration};

const PRODUCER_BUDGET: Duration = Duration::from_secs(5);
const ALARM_BUDGET: Duration = Duration::from_secs(10);

pub(super) fn run(stderr: &mut impl Write) -> u8 {
    let Some(config) = Configuration::read(|name| std::env::var_os(name)) else {
        let _ = stderr.write_all(b"posture digest: HOME is not set\n");
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
    mut clock: impl Clock,
    runner: impl CommandRunner,
    alarm: impl CommandRunner,
    stderr: &mut impl Write,
) -> u8 {
    // THE CLOCK IS READ BEFORE THE SPOOL IS TOUCHED. It names the day in the
    // title and stamps the claimed batch, so a clock that cannot answer leaves
    // the batch where it is rather than claiming one this run cannot finish
    // naming.
    let Ok(now) = clock.now() else {
        let _ = stderr.write_all(b"posture digest: the clock did not answer\n");
        return 1;
    };
    if let Err(error) = prepare_spool_directory(&config.store) {
        let _ = writeln!(
            stderr,
            "posture digest: the spool directory is unusable: {error}"
        );
        return 1;
    }
    let spool = DigestSpoolFile::new(config.store, now.seconds, std::process::id());
    let mut sink = alert_sink(
        config.delivery,
        runner,
        LastResortBanner::new(alarm, config.alarm),
        &mut *stderr,
    );
    let report = BuildDigest {
        spool: &spool,
        sink: &mut sink,
        utc_day: &now.utc_day,
        occurred_at: Some(now.seconds),
        limits: config.limits,
    }
    .run();
    // A DROPPED LINE IS A FINDING NOBODY WILL EVER READ. One torn line no
    // longer wedges the digest, and this is what keeps that from being a
    // silent trade: the count lands in the log beside the run that made it.
    if report.dropped > 0 {
        let _ = writeln!(
            stderr,
            "posture digest: dropped {} unreadable line(s) from the batch",
            report.dropped
        );
    }
    match report.outcome {
        // A SPOOL THAT COULD NOT BE TAKEN IS NOT A QUIET DAY, and exiting 0
        // with nothing on stderr made the two identical. The batch is still on
        // disk for the next run's sweep, so nothing is lost, but the run says
        // what it could not read and fails. Nonzero is safe here: this
        // LaunchAgent runs on a calendar interval with no KeepAlive, so nothing
        // retries it, and the uptime watchdog pages only after two failing runs
        // in a row.
        DigestOutcome::NotClaimed(failure) => {
            let _ = writeln!(
                stderr,
                "posture digest: the spool could not be claimed: {failure}"
            );
            1
        }
        // A LOST DAILY DIGEST IS LOW STAKES and the batch is already back in
        // the spool for tomorrow, so a refused send is not this run's failure
        // to report. The engine raises its own alarm when the pipeline itself
        // broke.
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
