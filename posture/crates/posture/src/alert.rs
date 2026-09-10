pub(super) mod configuration;
use configuration::Configuration;
use posture_adapters::{
    AllowlistText, BatchJudge, Collaborators, CursorFile, DigestAppendFile, LastResortBanner,
    PnsProducer, ResultsFile, ResultsRow, SingleRunLock, SystemClock, SystemRunner,
};
use posture_application::{Clock, JudgeOutcome, JudgeResults};
use std::{io::Write, time::Duration};

const PRODUCER_BUDGET: Duration = Duration::from_secs(5);
const ALARM_BUDGET: Duration = Duration::from_secs(10);

/// `posture alert`: judge whatever osquery has written since the cursor.
///
/// EXIT 0 ON EVERY PATH THE MACHINE CAN REACH, including a page that could not
/// be delivered. The batch WAS processed, the failure is already surfaced by
/// the sink's own alarm, and a nonzero exit here would false-trip the uptime
/// watchdog's crash-loop check. The cursor simply stays put and the next run
/// re-reads the same rows.
pub(super) fn run(stderr: &mut impl Write) -> u8 {
    let Some(config) = Configuration::read(|name| std::env::var_os(name)) else {
        let _ = stderr.write_all(b"posture alert: HOME is not set\n");
        return 1;
    };
    execute(config, SystemClock, stderr)
}

fn execute(config: Configuration, mut clock: impl Clock, stderr: &mut impl Write) -> u8 {
    // A CLOCK THAT CANNOT ANSWER STILL JUDGES. Unlike the digest, whose title
    // names a day, the only thing the clock supplies here is the timestamp on a
    // spooled row, and a spool line with no timestamp is worth more than a
    // batch nobody looked at.
    let now = clock.now().ok();
    // A FULL INSTANT, not the day. A digest groups a day's findings and prints
    // them in order, so a spool line stamped with only the date sorts against
    // every other line from that day arbitrarily.
    let stamp = now.as_ref().map(instant).unwrap_or_default();

    let log = ResultsFile::new(config.log);
    let cursor = CursorFile::new(config.cursor.clone());
    let lock = SingleRunLock::beside(&config.cursor);
    let spool = DigestAppendFile::new(config.spool);
    let allowlist = AllowlistText::read(&config.allowlist, &config.home);

    let mut sink = PnsProducer::new(
        SystemRunner::per_command(PRODUCER_BUDGET),
        config.pns,
        Some(
            String::from("posture")
                .try_into()
                .expect("the fixed posture route is valid"),
        ),
        LastResortBanner::new(SystemRunner::per_command(ALARM_BUDGET), config.alarm),
    );

    // THE COLLABORATORS ARE THE HONEST NOT-YET. The enricher and the
    // known-good manifest reader are separate cutovers, so this run vouches
    // for nothing and inspects nothing: every file event reaches the gate as
    // unvouched, which is the direction that PAGES rather than the one that
    // goes quiet. Wiring them is the next slice, and until then a page that
    // should have been suppressed is noise, never a page that should have
    // fired and did not.
    let mut vouches = |_: &str| false;
    let mut inspect = |_: &str| None;
    let mut triage = |_: &ResultsRow| None;
    let allowlist_path = config.allowlist.to_string_lossy().into_owned();
    let mut judge = BatchJudge {
        home: &config.home,
        allowlist_path: &allowlist_path,
        allowlist: allowlist.as_ref(),
        spool: &spool,
        now: &stamp,
        collaborators: Collaborators {
            vouches: &mut vouches,
            inspect: &mut inspect,
            triage: &mut triage,
        },
    };

    let outcome = JudgeResults {
        lock: &lock,
        log: &log,
        cursor: &cursor,
        judge: &mut judge,
        sink: &mut sink,
        occurred_at: now.map(|reading| reading.seconds),
    }
    .run();

    if let JudgeOutcome::Retained = outcome {
        let _ = stderr.write_all(
            b"posture alert: the page could not be delivered; the cursor stays put for a retry\n",
        );
    }
    0
}

/// `<utc_day>T<hh>:<mm>:<ss>Z`, built from the two facts the clock reports.
///
/// The day already came from `gmtime_r`, so the time of day is the remainder of
/// the same epoch second rather than a second call that could land in the next
/// day from the first.
fn instant(reading: &posture_application::WallTime) -> String {
    let seconds_today = reading.seconds % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        reading.utc_day,
        seconds_today / 3600,
        (seconds_today % 3600) / 60,
        seconds_today % 60
    )
}

#[cfg(test)]
#[path = "alert/tests.rs"]
mod tests;
