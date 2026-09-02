//! The uu binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The environment and the config are read once at
//! this edge, every decision is delegated to the library, and the three
//! process boundaries (the lane subjects, the pns engine, the hermes gateway)
//! each have exactly one implementation, right here.
//!
//! EXIT CODES SAY WHO FAILED. A lane that failed does not fail the run (0),
//! because the record is what reports it and the next attempt is a week away.
//! A config uu could not read, or a lane the operator asked for and did not
//! get, is uu failing (1). An argument uu does not serve is usage (2).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pns::channels::hermes::{SignedPost, UreqSignedPost, delivered, outcome_line, sign};
use unattended_upgrades::alert::{Alerter, alert_argv, alert_summary};
use unattended_upgrades::config::{
    Config, ConfigError, LANE_TYPES, LoadOutcome, Records, config_path, load_config,
};
use unattended_upgrades::lanes::{
    CommandRunner, LaneReport, enabled_lanes, failure_reason, run_lane,
};
use unattended_upgrades::record::{
    AGENT, Marker, gap_line, marker_contents, parse_marker, record_body, record_detail,
    record_state,
};
use unattended_upgrades::schedule::{DEFAULT_LABEL, render_plist};

fn main() {
    // Die on a closed pipe the way every other unix tool does. Rust ignores
    // SIGPIPE and turns the failed write into a panic instead, so `uu doctor |
    // head` would print a backtrace over the output it just produced.
    // SAFETY: restoring a signal's default disposition, before any thread or
    // handler of this program's own exists.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    std::process::exit(dispatch());
}

/// The whole CLI. A slice match rather than a chain, so an extra word is an
/// error instead of an argument nothing reads.
fn dispatch() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["run"] => run_mode(None),
        ["run", lane] => run_mode(Some(lane)),
        ["doctor"] => doctor_mode(),
        ["schedule", "render"] => schedule_mode(),
        [] => usage("no command given"),
        [command, ..] => usage(&format!("unknown command `{command}`")),
    }
}

/// What uu serves, printed on stderr with a non-zero exit. Never a silent
/// fallthrough to help with exit 0.
fn usage(problem: &str) -> i32 {
    eprintln!(
        "uu: {problem}\n\
         usage:\n  \
           uu run [<lane>]     run every enabled lane, or just one\n  \
           uu doctor           what this config turns on, and what it cannot reach\n  \
           uu schedule render  the launchd job for the configured day and time\n\
         lane types: {}",
        LANE_TYPES.join(", ")
    );
    2
}

/// How long one signed record POST may take. Nobody is waiting on the answer,
/// so this only bounds how long the job lingers on a gateway that stopped
/// listening; it matches the deadline pns gives its own unwatched posts.
const RECORD_DEADLINE: Duration = Duration::from_secs(10);

// --- the run ---------------------------------------------------------------

fn run_mode(only: Option<&str>) -> i32 {
    let Some(home) = home() else {
        eprintln!("uu: HOME is not set, so there is no config to read");
        return 1;
    };

    let path = config_path(&home);
    let config = match loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            // A bare run on a configless machine is clean by design. A lane
            // asked for BY NAME is a request, and one no file declares did
            // not run, so it is refused the way an undeclared name is below.
            if let Some(lane) = only {
                eprintln!(
                    "uu: no config at {}, so no lane `{lane}` is declared",
                    path.display()
                );
                return 1;
            }
            println!(
                "uu: no config at {}; nothing is enabled and nothing was updated",
                path.display()
            );
            return 0;
        }
        Err(code) => return code,
    };

    // The header is captured BEFORE the lanes run and before the marker is
    // rewritten: a gap sampled at delivery reports zero on every run. A clock
    // that cannot be read renders as the epoch itself, which is a date no
    // reader mistakes for a real one.
    let started = now_epoch().unwrap_or(0);
    let marker_path = marker_path(&home);
    let gap = gap_line(
        &read_marker(&marker_path),
        &marker_path.display().to_string(),
        started,
    );

    let runner = SystemRunner;
    let mut reports: Vec<LaneReport> = Vec::new();
    for name in enabled_lanes(&config) {
        if only.is_some_and(|wanted| wanted != name) {
            continue;
        }
        // CONTINUE ON FAILURE: nothing here inspects the report before moving
        // to the next lane.
        if let Some(report) = run_lane(name, &config, &runner) {
            reports.push(report);
        }
    }
    if let Some(lane) = only
        && reports.is_empty()
    {
        eprintln!(
            "uu: lane `{lane}` has no `[lanes.{lane}]` block in {}",
            path.display()
        );
        return 1;
    }

    let failures: usize = reports.iter().map(|report| report.failures).sum();
    let detail = record_detail(&host(), &iso(started), &gap, &reports);
    print!("{detail}");

    let engine = config.alerts.as_ref().map(|alerts| alerts.binary.clone());
    for report in reports.iter().filter(|report| report.failures > 0) {
        send_alert(
            &PnsAlerter,
            engine.as_deref(),
            &report.name,
            &alert_summary(report),
        );
    }

    // A RECORD THE GATEWAY NEVER RECEIVED IS A FAILED RUN, even when every
    // lane passed. The entry is the whole point of the week's work, and the
    // marker is what the NEXT entry measures its gap from: stamping a success
    // nothing recorded would have that entry claim a gap from a week no one
    // can read. With no `[records]` block nothing was owed, so nothing is
    // lost.
    let record_lost = match config.records.as_ref() {
        Some(records) => !deliver_record(
            &UreqSignedPost,
            &PnsAlerter,
            records,
            records_body(failures, &detail),
            engine.as_deref(),
        ),
        None => {
            println!("uu: no [records] block; this run was logged here and nowhere else");
            false
        }
    };

    // THE MARKER MOVES ONLY ON A CLEAN RUN, so the next entry's gap measures
    // the last time everything actually worked rather than the last time uu
    // woke up.
    //
    // AND IT STAMPS THE MOMENT THE RUN FINISHED, read here rather than reused
    // from the header. Lanes have no upper bound, so the header's instant can
    // be an hour old by now, and every following gap would carry that hour on
    // top of its own. A clock that will not answer at this instant leaves the
    // marker alone: an unmoved marker overstates the next gap, while a
    // guessed timestamp understates it silently.
    if failures == 0 && !record_lost {
        match now_epoch() {
            Some(finished) => write_marker(&marker_path, finished),
            None => eprintln!(
                "uu: this clock could not be read at the end of the run, so the successful-run \
                 timestamp at {} was left as it was; the next entry measures its gap from the \
                 run before this one",
                marker_path.display()
            ),
        }
    }
    0
}

fn records_body(failures: usize, detail: &str) -> String {
    record_body(record_state(failures), &host(), detail)
}

// --- the doctor ------------------------------------------------------------

fn doctor_mode() -> i32 {
    let Some(home) = home() else {
        eprintln!("uu: HOME is not set, so there is no config to read");
        return 1;
    };
    let path = config_path(&home);
    println!("uu: config {}", path.display());
    let config = match loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("uu: no config file; every lane is off");
            Config::default()
        }
        Err(code) => return code,
    };

    if config.lanes.is_empty() {
        println!("uu: lanes: none declared");
    } else {
        for (name, kind) in &config.lanes {
            println!("uu: lane {name}: on ({})", kind.type_name());
        }
    }
    match config.records.as_ref() {
        // THE KEY IS NEVER PRINTED, only whether there is one.
        Some(records) => println!("uu: records: on, posting to {} (key set)", records.url),
        None => println!("uu: records: off, no [records] block"),
    }
    match config.alerts.as_ref() {
        Some(alerts) => {
            let reachable = match resolve(&alerts.binary) {
                Some(found) => format!("found at {}", found.display()),
                None => "NOT FOUND; failures will be logged and nothing else".to_string(),
            };
            println!("uu: alerts: on via `{}`, {reachable}", alerts.binary);
        }
        None => println!("uu: alerts: off, no [alerts] block"),
    }
    let schedule = config.schedule;
    println!(
        "uu: schedule: weekday {} at {:02}:{:02} (this feeds `uu schedule render` only)",
        schedule.weekday, schedule.hour, schedule.minute
    );
    let marker_path = marker_path(&home);
    println!(
        "uu: {}",
        gap_line(
            &read_marker(&marker_path),
            &marker_path.display().to_string(),
            now_epoch().unwrap_or(0)
        )
    );
    0
}

// --- the schedule ----------------------------------------------------------

fn schedule_mode() -> i32 {
    let Some(home) = home() else {
        eprintln!("uu: HOME is not set, so there is no config to read");
        return 1;
    };
    let config = match loaded(&config_path(&home)) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(code) => return code,
    };
    print!("{}", render_plist(DEFAULT_LABEL, &home, config.schedule));
    0
}

// --- the edges -------------------------------------------------------------

/// The config, or `None` for a machine that has not written one. A refusal is
/// printed here and returned as an exit code, because every mode answers it
/// the same way: loudly, and without guessing.
fn loaded(path: &Path) -> Result<Option<Config>, i32> {
    match load_config(path) {
        Ok(LoadOutcome::Loaded(config)) => Ok(Some(config)),
        Ok(LoadOutcome::Missing) => Ok(None),
        Err(error) => {
            let what = match error {
                ConfigError::Malformed(_) => "is not valid TOML",
                ConfigError::Invalid(_) => "is not a config uu can use",
                ConfigError::Unreadable(_) => "could not be read",
            };
            eprintln!("uu: {} {what}: {}", path.display(), error.detail());
            Err(1)
        }
    }
}

fn home() -> Option<String> {
    std::env::var("HOME").ok().filter(|home| !home.is_empty())
}

fn marker_path(home: &str) -> PathBuf {
    Path::new(home).join(".local/state/uu/last-success")
}

/// This instant in epoch seconds, or `None` for a clock set before 1970. The
/// caller decides what an unreadable clock means: the header prints it, the
/// marker refuses to move on it.
fn now_epoch() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs() as i64)
}

/// ISO 8601 UTC, computed from the same epoch the gap is, so the two figures
/// in one entry can never be sampled at different instants.
fn iso(epoch: i64) -> String {
    let mut when = std::mem::MaybeUninit::<libc::tm>::uninit();
    let seconds = epoch as libc::time_t;
    // SAFETY: gmtime_r writes into the caller's own tm and returns null only
    // when it wrote nothing, which is the branch below.
    let filled = unsafe { libc::gmtime_r(&seconds, when.as_mut_ptr()) };
    if filled.is_null() {
        return format!("epoch {epoch}");
    }
    let when = unsafe { when.assume_init() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        when.tm_year + 1900,
        when.tm_mon + 1,
        when.tm_mday,
        when.tm_hour,
        when.tm_min,
        when.tm_sec
    )
}

/// The machine this record is about. The channel aggregates unattended jobs
/// from more than one host, so an entry that does not name its host is not
/// investigable.
fn host() -> String {
    let mut name = [0_i8; 256];
    // SAFETY: the buffer and its length are this frame's own, and the result
    // is only read after a success, truncated at the first NUL.
    let read = unsafe { libc::gethostname(name.as_mut_ptr(), name.len()) };
    if read != 0 {
        return "unknown-host".to_string();
    }
    let bytes: Vec<u8> = name
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect();
    let full = String::from_utf8_lossy(&bytes).to_string();
    // The short name, matching `hostname -s`: a bonjour suffix says nothing
    // an entry needs.
    let short = full.split('.').next().unwrap_or_default().to_string();
    if short.is_empty() {
        "unknown-host".to_string()
    } else {
        short
    }
}

/// The marker, with an ABSENT path told from a BROKEN LINK. Both read
/// NotFound, and they are opposite states: nothing there is a machine that has
/// never finished a run, while a link whose target went away is bookkeeping
/// that stopped resolving and has to be said out loud.
fn read_marker(path: &Path) -> Marker {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_marker(&text),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Marker::NeverRecorded
        }
        Err(_) => Marker::Unreadable,
    }
}

/// Best effort, and never silent: a job must not fail because it could not
/// write its own bookkeeping, but a failure to write would have the next entry
/// measure its gap from a run that did not happen.
fn write_marker(path: &Path, epoch: i64) {
    let written = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, marker_contents(epoch, &iso(epoch))));
    if let Err(error) = written {
        eprintln!(
            "uu: could not record the successful-run timestamp at {}: {error}; the next entry \
             will report a stale or absent gap",
            path.display()
        );
    }
}

/// Where a command name resolves on this PATH, or `None`. An absolute or
/// relative path is answered from the filesystem directly, the way a shell
/// does.
fn resolve(command: &str) -> Option<PathBuf> {
    let runnable = |path: &Path| path.is_file();
    if command.contains('/') {
        let path = PathBuf::from(command);
        return runnable(&path).then_some(path);
    }
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| Path::new(entry).join(command))
        .find(|candidate| runnable(candidate))
}

/// The lane subjects. No deadline, matching the shell job this replaces: a
/// plugin install has no honest upper bound and launchd is what notices a job
/// that never ends.
struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        match Command::new(program).args(args).output() {
            Err(error) => Err(format!("could not run {program}: {error}")),
            Ok(output) if output.status.success() => {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            }
            // WHAT IT PRINTED, not only how it ended. `output` captured stderr
            // either way, the child is gone by the time the record is
            // composed, and a weekly job's own log may have rotated before
            // anyone reads it.
            Ok(output) => Err(failure_reason(
                &match output.status.code() {
                    Some(code) => format!("exit {code}"),
                    None => "killed by a signal".to_string(),
                },
                &String::from_utf8_lossy(&output.stderr),
            )),
        }
    }
}

/// The pns engine, as a client: flags in, nothing read back.
struct PnsAlerter;

impl Alerter for PnsAlerter {
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
        match Command::new(binary).args(args).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("`{binary}` answered {status}")),
            Err(error) => Err(format!("`{binary}` could not be run: {error}")),
        }
    }
}

/// One alert, FAIL OPEN at every rung: no `[alerts]` block, an engine that is
/// not there, and an engine that refused are each stated here and none of them
/// ends the run.
fn send_alert(alerter: &dyn Alerter, engine: Option<&str>, lane: &str, summary: &str) {
    let Some(binary) = engine else {
        println!("uu: no [alerts] block; `{lane}: {summary}` was logged and nothing else");
        return;
    };
    let argv = alert_argv(&host(), lane, summary);
    if let Err(why) = alerter.alert(binary, &argv) {
        println!("uu: the alert for `{lane}` was NOT delivered ({why}); it is logged here instead");
    }
}

/// The record, posted in process, ANSWERING WHETHER IT LANDED. The caller
/// needs that verdict: an entry the gateway never received is a failed run,
/// whatever the lanes did.
///
/// FAIL LOUD: a refused delivery is printed AND alerted, because a silent
/// record channel is indistinguishable from a machine whose jobs stopped
/// running, which is the one failure the record cannot report about itself.
///
/// BOTH PROCESS BOUNDARIES ARRIVE AS TRAITS, never the concrete client: a
/// refused delivery can only be exercised through a real socket failure
/// otherwise, and the alert it fires is invisible to anything but a real
/// engine. `run_mode` is the only caller and hands in the production pair.
fn deliver_record(
    post: &dyn SignedPost,
    alerter: &dyn Alerter,
    records: &Records,
    body: String,
    engine: Option<&str>,
) -> bool {
    let Some(signature) = sign(&records.key, &body) else {
        println!("uu: the [records] key is empty, so nothing could be signed or posted");
        return false;
    };
    let outcome = post.post(&records.url, &body, &signature, Some(RECORD_DEADLINE));
    println!("uu: {}", outcome_line(outcome));
    if delivered(outcome) {
        return true;
    }
    send_alert(
        alerter,
        engine,
        AGENT,
        &format!(
            "the weekly record could NOT be delivered to {} ({}); until this is fixed that \
             channel is silent for a reason that has nothing to do with the jobs it reports on",
            records.url,
            outcome_line(outcome)
        ),
    );
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path of this test's own under the temp directory, with nothing at it.
    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("uu-main-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_path_with_no_marker_at_it_is_a_machine_that_never_recorded_a_run() {
        assert_eq!(read_marker(&scratch("absent")), Marker::NeverRecorded);
    }

    #[test]
    fn a_failed_command_reports_what_it_printed_and_not_only_its_status() {
        // The one place stderr is still readable is here: the child is gone by
        // the time the record is composed, and a weekly job's log may have
        // rotated before anyone reads it.
        let failure = SystemRunner
            .run(
                "/bin/sh",
                &["-c", "printf 'no such repository\\n' >&2; exit 3"],
            )
            .expect_err("this command fails");
        assert!(failure.contains("exit 3"), "{failure}");
        assert!(failure.contains("no such repository"), "{failure}");
    }

    #[test]
    fn a_dangling_marker_symlink_is_unreadable_rather_than_never_recorded() {
        // A broken link reads NotFound exactly like an absent path, and the
        // two are opposite states: nothing recorded yet is a fresh machine,
        // while a link whose target went away is bookkeeping that STOPPED
        // resolving. Read as "never recorded" it reports a fresh machine
        // forever and no gap is ever measured again.
        let link = scratch("marker-dangling");
        std::os::unix::fs::symlink("uu-absent-target", &link).expect("the link");
        let marker = read_marker(&link);
        std::fs::remove_file(&link).ok();
        assert_eq!(marker, Marker::Unreadable);
    }

    // --- the record and alert seams --------------------------------------

    use pns::channels::hermes::PostOutcome;
    use std::cell::RefCell;

    /// A `SignedPost` stub that always answers the same fixed outcome. It
    /// never touches a socket, so both directions below run in well under a
    /// second and neither depends on a real gateway being up or down.
    struct AnswerWith(PostOutcome);

    impl SignedPost for AnswerWith {
        fn post(
            &self,
            _url: &str,
            _body: &str,
            _signature_hex: &str,
            _deadline: Option<Duration>,
        ) -> PostOutcome {
            self.0
        }
    }

    /// An `Alerter` that records every call instead of spawning anything, so
    /// a test can assert whether the alert path fired at all.
    #[derive(Default)]
    struct SpyAlerter {
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Alerter for SpyAlerter {
        fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push((binary.to_string(), args.to_vec()));
            Ok(())
        }
    }

    fn stub_records() -> Records {
        Records {
            url: "http://127.0.0.1:0/wherever".to_string(),
            key: "k".to_string(),
        }
    }

    #[test]
    fn a_refused_post_reports_failure_and_alerts_through_the_given_alerter() {
        let spy = SpyAlerter::default();
        let delivered = deliver_record(
            &AnswerWith(PostOutcome::NoResponse),
            &spy,
            &stub_records(),
            "body".to_string(),
            Some("engine"),
        );
        assert!(!delivered);
        assert_eq!(spy.calls.borrow().len(), 1, "{:?}", spy.calls.borrow());
    }

    #[test]
    fn a_delivered_post_reports_success_and_never_touches_the_alerter() {
        let spy = SpyAlerter::default();
        let delivered = deliver_record(
            &AnswerWith(PostOutcome::Status(200)),
            &spy,
            &stub_records(),
            "body".to_string(),
            Some("engine"),
        );
        assert!(delivered);
        assert!(spy.calls.borrow().is_empty(), "{:?}", spy.calls.borrow());
    }
}
